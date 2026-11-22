// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Application metadata and command-line dispatch.

use getoptsargs::prelude::*;
use std::env;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::Command;

fn extra_help(writer: &mut dyn Write) -> io::Result<()> {
    writeln!(writer, "Commands:")?;
    writeln!(writer, "    help                show command-line usage information")?;
    writeln!(writer, "    version             show version information")
}

fn app_setup(builder: Builder) -> Builder {
    builder
        .copyright("Copyright 2026 Julio Merino")
        .extra_help(extra_help)
        .homepage(env!("CARGO_PKG_REPOSITORY"))
        .manpage("jobmarshal", "1")
}

fn app_main(_matches: Matches) -> Result<i32> {
    Ok(0)
}

/// Runs the command-line application and returns its exit status.
pub fn main() -> i32 {
    let mut args = env::args_os().skip(1);
    if let Some(command) = args.next() {
        let option = if command == "help" {
            Some("--help")
        } else if command == "version" {
            Some("--version")
        } else {
            None
        };
        if let Some(option) = option {
            if args.next().is_some() {
                eprintln!("Usage error: {command:?} does not accept arguments");
                eprintln!("Type `jobmarshal help` or `man 1 jobmarshal` for more information");
                return 2;
            }
            // getoptsargs enables command mode only after registering a custom command.  Until
            // there is one, use its built-in options through a fresh process: Builder takes Args,
            // so translated arguments cannot be passed directly.
            let error = match env::current_exe() {
                Ok(path) => Command::new(path).arg(option).exec(),
                Err(error) => error,
            };
            eprintln!("jobmarshal: cannot execute built-in command: {error}");
            return 1;
        }
    }
    app_setup(Builder::new("Jobmarshal", env!("CARGO_PKG_VERSION"), env::args())).run(app_main)
}
