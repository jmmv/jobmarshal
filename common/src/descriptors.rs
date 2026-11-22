// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Descriptor lifetime across transparent command execution.

use anyhow::Result;
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use std::os::fd::AsFd;

/// Makes a lease or participant lock survive exec in the command and descendants.
pub fn inherit(descriptor: &impl AsFd) -> Result<()> {
    let flags = FdFlag::from_bits_truncate(fcntl(descriptor, FcntlArg::F_GETFD)?);
    fcntl(descriptor, FcntlArg::F_SETFD(flags & !FdFlag::FD_CLOEXEC))?;
    Ok(())
}
