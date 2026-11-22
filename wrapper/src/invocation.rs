// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Direct commands and identity-based PATH anchors.

use anyhow::{Context, Result, bail};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) struct Invocation {
    pub(crate) arg0: OsString,
    pub(crate) arguments: Vec<OsString>,
    pub(crate) executable: PathBuf,
    pub(crate) server: PathBuf,
}

fn resolve(invocation: &Path) -> Result<(PathBuf, PathBuf)> {
    let name = invocation.file_name().context("invalid invocation name")?;
    let identity = fs::metadata(env::current_exe().context("cannot identify running wrapper")?)?;
    let path = env::var_os("PATH").context("PATH is not set")?;
    let directories: Vec<_> = env::split_paths(&path)
        .map(
            |directory| {
                if directory.as_os_str().is_empty() { PathBuf::from(".") } else { directory }
            },
        )
        .collect();
    let is_wrapper = |metadata: &fs::Metadata| {
        identity.dev() == metadata.dev() && identity.ino() == metadata.ino()
    };
    let (anchor, start) = if let Some(index) = directories.iter().position(|directory| {
        fs::metadata(directory.join(name)).is_ok_and(|metadata| is_wrapper(&metadata))
    }) {
        (directories[index].clone(), index + 1)
    } else {
        // An explicit path identifies the anchor even when its directory is absent from PATH.
        let anchor = invocation
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .context("cannot locate this wrapper's identity in PATH")?;
        (fs::canonicalize(anchor).context("cannot identify wrapper directory")?, 0)
    };
    let server =
        if jobmarshal_common::wrappers::directory().is_ok_and(|root| anchor == root.join("bin")) {
            PathBuf::from("jobmarshal")
        } else {
            anchor.join("jobmarshal")
        };
    for directory in &directories[start..] {
        let candidate = directory.join(name);
        let Ok(metadata) = fs::metadata(&candidate) else { continue };
        if !is_wrapper(&metadata)
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Ok((candidate, server));
        }
    }
    bail!("no real {} executable after the wrapper in PATH", name.to_string_lossy())
}

pub(crate) fn parse() -> Result<Invocation> {
    let mut args = env::args_os();
    let arg0 = args.next().context("missing invocation name")?;
    let name = Path::new(&arg0).file_name().context("invalid invocation name")?;
    let (arg0, executable, server) = if name == "jobmarshal-wrapper" {
        let command = args.next().context("usage: jobmarshal-wrapper COMMAND [ARG ...]")?;
        let sibling = env::current_exe()?.with_file_name("jobmarshal");
        let server = if sibling.is_file() { sibling } else { PathBuf::from("jobmarshal") };
        (command.clone(), PathBuf::from(command), server)
    } else {
        let (executable, server) = resolve(Path::new(&arg0))?;
        (arg0, executable, server)
    };
    Ok(Invocation { arg0, arguments: args.collect(), executable, server })
}
