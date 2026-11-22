// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Location and naming of persistent per-user wrappers.

use anyhow::{Context, Result, ensure};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// Returns the application data directory without creating it.
pub fn directory() -> Result<PathBuf> {
    let base =
        xdg::BaseDirectories::new().get_data_home().context("cannot locate XDG data home")?;
    ensure!(base.is_absolute(), "XDG data home must be absolute");
    ensure!(!base.as_os_str().as_bytes().contains(&b':'), "XDG data home cannot contain ':'");
    Ok(base.join("jobmarshal").components().collect())
}

/// Reports whether a name is a supported command alias.
pub fn valid_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
        && name != "jobmarshal"
        && name != "jobmarshal-wrapper"
}
