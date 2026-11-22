// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! OS-string command dispatch through the relocatable installed wrapper.

use crate::wrappers;
use anyhow::{Context, Result};
use getoptsargs::prelude::*;
use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::Command;

fn extra_help(writer: &mut dyn Write) -> io::Result<()> {
    writeln!(
        writer,
        "An optional -- may precede COMMAND.  All later arguments belong to the child."
    )
}

pub(crate) fn options(builder: CommandBuilder) -> CommandBuilder {
    builder
        .posarg("COMMAND", "command to execute (optionally preceded by --)")
        .trailarg("ARG", 0, usize::MAX, "arguments passed unchanged to the command")
        .extra_help(extra_help)
}

fn execute(command: OsString, arguments: impl Iterator<Item = OsString>) -> Result<()> {
    let wrapper = wrappers::installed()?;
    Err(Command::new(&wrapper).arg(command).args(arguments).exec())
        .with_context(|| format!("cannot execute wrapper {}", wrapper.display()))
}

pub(crate) fn dispatch(_app: Matches, command: Matches) -> Result<i32> {
    execute(command.arg_pos("COMMAND").into(), command.arg_trail().iter().map(OsString::from))?;
    Ok(0)
}

pub(crate) fn intercept() -> Option<i32> {
    let mut arguments = env::args_os().skip(1);
    if arguments.next().is_none_or(|argument| argument != "run") {
        return None;
    }
    // getoptsargs accepts UTF-8 arguments and parses options.  Child arguments must bypass both.
    let command = arguments
        .next()
        .and_then(|argument| if argument == "--" { arguments.next() } else { Some(argument) });
    let Some(command) = command else {
        eprintln!("Usage error: Missing command for `jobmarshal run`");
        eprintln!("Type `jobmarshal help run` or `man 1 jobmarshal` for more information");
        return Some(2);
    };
    Some(match execute(command, arguments) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("jobmarshal: {error:#}");
            1
        }
    })
}
