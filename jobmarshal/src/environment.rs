// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Environment assignments shared by detached startup and session initialization.

use anyhow::Result;
use jobmarshal_common::wrappers;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn export(output: &mut Vec<u8>, name: &str, value: &OsStr) {
    output.extend_from_slice(format!("export {name}='").as_bytes());
    for byte in value.as_bytes() {
        if *byte == b'\'' {
            output.extend_from_slice(b"'\\''");
        } else {
            output.push(*byte);
        }
    }
    output.extend_from_slice(b"'\n");
}

pub(crate) fn print(runtime: &Path) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    export(&mut output, "JOBMARSHAL_RUNTIME_DIR", runtime.as_os_str());
    let bin = wrappers::directory()?.join("bin");
    let path = env::var_os("PATH");
    if !path.as_ref().is_some_and(|path| env::split_paths(path).any(|entry| entry == bin)) {
        let mut value = OsString::from(bin);
        if let Some(path) = path {
            value.push(":");
            value.push(path);
        }
        export(&mut output, "PATH", &value);
    }
    Ok(output)
}
