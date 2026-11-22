// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Direct commands and identity-based PATH anchors.

use anyhow::{Context, Result, bail, ensure};
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

fn resolve(name: &Path) -> Result<(PathBuf, PathBuf)> {
    let identity = fs::metadata(env::current_exe().context("cannot identify running wrapper")?)?;
    let path = env::var_os("PATH").context("PATH is not set")?;
    let mut anchor = None;
    for directory in env::split_paths(&path) {
        let directory =
            if directory.as_os_str().is_empty() { PathBuf::from(".") } else { directory };
        let candidate = directory.join(name);
        let Ok(metadata) = fs::metadata(&candidate) else { continue };
        if identity.dev() == metadata.dev() && identity.ino() == metadata.ino() {
            if anchor.is_none() {
                anchor = Some(directory);
            }
        } else if metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
            && let Some(anchor) = &anchor
        {
            return Ok((candidate, anchor.join("jobmarshal")));
        }
    }
    ensure!(anchor.is_some(), "cannot locate this wrapper's identity in PATH");
    bail!("no real {} executable after the wrapper in PATH", name.display())
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
        let (executable, server) = resolve(Path::new(name))?;
        (arg0, executable, server)
    };
    Ok(Invocation { arg0, arguments: args.collect(), executable, server })
}
