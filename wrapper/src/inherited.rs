// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Import native allocations before opening unrelated descriptors.

use anyhow::{Result, bail, ensure};
use jobserver::Client;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::process::Command;

pub(crate) const FLAG_VARIABLES: [&str; 3] = ["CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"];

/// This must be the first operation that can open descriptors in the wrapper.
#[allow(unsafe_code)]
pub(crate) fn import() -> Result<Option<Client>> {
    // SAFETY: execution::run calls this before runtime discovery, resolution, or other I/O.
    // The upstream implementation validates pipe descriptors and duplicates them for ownership.
    let inherited = unsafe { Client::from_env_ext(true) };
    match inherited.client {
        Ok(client) => {
            if let Some((_, flags)) = inherited.var {
                let flags = flags.to_string_lossy();
                let authentication = ["--jobserver-auth=", "--jobserver-fds="]
                    .iter()
                    .find_map(|prefix| flags.rsplit_once(prefix).map(|(_, auth)| auth))
                    .and_then(|auth| auth.split(' ').next());
                if let Some(path) = authentication.and_then(|auth| auth.strip_prefix("fifo:")) {
                    ensure!(
                        fs::metadata(path)?.file_type().is_fifo(),
                        "inherited jobserver path is not a FIFO"
                    );
                }
            }
            Ok(Some(client))
        }
        Err(error) => {
            if let Some((variable, flags)) = inherited.var
                && flags.to_string_lossy().contains("--jobserver-")
            {
                bail!("invalid inherited jobserver in {variable:?}: {error}");
            }
            Ok(None)
        }
    }
}

pub(crate) fn flags(command: &mut Command, authentication: &OsStr) {
    for name in FLAG_VARIABLES {
        let mut flags = env::var_os(name).unwrap_or_default();
        flags.push(" ");
        flags.push(authentication);
        command.env(name, flags);
    }
}

pub(crate) fn configure(client: &Client, command: &mut Command) {
    client.configure(command);
    let authentication: OsString = command
        .get_envs()
        .find_map(|(name, value)| {
            (name == "CARGO_MAKEFLAGS").then(|| value.unwrap().to_os_string())
        })
        .unwrap();
    flags(command, &authentication);
}
