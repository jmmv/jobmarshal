// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Transparent exec with an inherited or newly admitted allocation.

use crate::{admission, inherited, invocation};
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Executes the requested command, returning only when setup or exec fails.
pub fn run() -> Result<()> {
    let inherited = inherited::import()?;
    let invocation = invocation::parse()?;
    let mut command = Command::new(&invocation.executable);
    command.arg0(&invocation.arg0).args(invocation.arguments);
    if let Some(client) = inherited {
        inherited::configure(&client, &mut command);
        return Err(command.exec()).context("cannot execute wrapped command");
    }
    let allocation = admission::acquire(&invocation.server)?;
    let mut flags = OsString::from("-j --jobserver-auth=fifo:");
    flags.push(&allocation.fifo);
    inherited::flags(&mut command, &flags);
    Err(command.exec()).context("cannot execute wrapped command")
}
