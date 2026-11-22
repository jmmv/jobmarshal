// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Environment assignments shared by detached startup and session initialization.

use anyhow::{Context, Result, ensure};
use jobmarshal_common::wrappers;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn assignment(output: &mut Vec<u8>, name: &str, value: &OsStr, format: &str) -> Result<()> {
    if format == "shell" {
        output.extend_from_slice(format!("export {name}='").as_bytes());
        for byte in value.as_bytes() {
            if *byte == b'\'' {
                output.extend_from_slice(b"'\\''");
            } else {
                output.push(*byte);
            }
        }
        output.extend_from_slice(b"'\n");
    } else {
        let value = value.to_str().context("systemd environment values must be UTF-8")?;
        ensure!(
            !value.contains(['\n', '\r']),
            "systemd environment values cannot contain newlines"
        );
        output.extend_from_slice(format!("{name}=\"").as_bytes());
        for byte in value.as_bytes() {
            if matches!(byte, b'\\' | b'"') {
                output.push(b'\\');
            }
            output.push(*byte);
        }
        output.extend_from_slice(b"\"\n");
    }
    Ok(())
}

pub(crate) fn print(runtime: &Path, format: &str) -> Result<Vec<u8>> {
    ensure!(matches!(format, "shell" | "systemd"), "unknown environment format: {format}");
    let mut output = Vec::new();
    assignment(&mut output, "JOBMARSHAL_RUNTIME_DIR", runtime.as_os_str(), format)?;
    let bin = wrappers::directory()?.join("bin");
    let path = env::var_os("PATH");
    if !path.as_ref().is_some_and(|path| env::split_paths(path).any(|entry| entry == bin)) {
        let mut value = OsString::from(bin);
        if let Some(path) = path {
            value.push(":");
            value.push(path);
        }
        assignment(&mut output, "PATH", &value, format)?;
    }
    Ok(output)
}
