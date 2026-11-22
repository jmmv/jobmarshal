// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Application metadata and command-line dispatch.

use crate::lifecycle;
use getoptsargs::prelude::*;
use jobmarshal_common::{capacity, client, protocol::Response, runtime::Paths};
use std::env;

fn no_options(builder: CommandBuilder) -> CommandBuilder {
    builder
}

fn start_options(builder: CommandBuilder) -> CommandBuilder {
    builder.optopt("j", "jobs", "shared job capacity (default: available CPUs)", "N")
}

fn start(_app: Matches, command: Matches) -> Result<i32> {
    let requested = capacity::requested(command.opt_str("jobs"))?;
    let paths = Paths::discover()?;
    lifecycle::start(&paths, requested)?;
    println!(
        "export JOBMARSHAL_RUNTIME_DIR='{}'",
        paths.directory.to_str().unwrap().replace('\'', "'\\''")
    );
    Ok(0)
}

fn status(_app: Matches, _command: Matches) -> Result<i32> {
    match client::status(&Paths::discover()?)? {
        Some(Response::Status { active, draining, jobs, pid, waiting }) => {
            println!(
                "running: pid={pid} jobs={jobs} active_builds={active} waiting_builds={waiting} draining={draining}"
            );
            Ok(0)
        }
        None => {
            println!("not running");
            Ok(1)
        }
        response => anyhow::bail!("unexpected status: {response:?}"),
    }
}

fn stop(_app: Matches, _command: Matches) -> Result<i32> {
    lifecycle::stop(&Paths::discover()?)?;
    Ok(0)
}

/// Runs the command-line application and returns its exit status.
pub fn main() -> i32 {
    Builder::new("Jobmarshal", env!("CARGO_PKG_VERSION"), env::args())
        .copyright("Copyright 2026 Julio Merino")
        .homepage(env!("CARGO_PKG_REPOSITORY"))
        .manpage("jobmarshal", "1")
        .cmd("start", "start the per-user jobmarshal", start_options, start)
        .cmd("status", "inspect the per-user jobmarshal", no_options, status)
        .cmd("stop", "stop the per-user jobmarshal", no_options, stop)
        .run(command_dispatcher)
}
