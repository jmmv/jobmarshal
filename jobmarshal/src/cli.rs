// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Application metadata and command-line dispatch.

use crate::{lifecycle, run, stats, wrappers};
use getoptsargs::prelude::*;
use jobmarshal_common::{capacity, client, protocol::Response, runtime::Paths};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

fn no_options(builder: CommandBuilder) -> CommandBuilder {
    builder
}

fn wrapper_options(builder: CommandBuilder) -> CommandBuilder {
    builder.posarg("NAME", "command basename to wrap")
}

fn wrapper_name(command: &Matches) -> Option<&str> {
    let name = command.arg_pos("NAME");
    if jobmarshal_common::wrappers::valid_name(name) {
        Some(name)
    } else {
        eprintln!("Usage error: Invalid wrapper basename: {name}");
        None
    }
}

fn add_wrapper(_app: Matches, command: Matches) -> Result<i32> {
    let Some(name) = wrapper_name(&command) else { return Ok(2) };
    wrappers::add(name)?;
    Ok(0)
}

fn list_wrappers(_app: Matches, _command: Matches) -> Result<i32> {
    for name in wrappers::list()? {
        println!("{name}");
    }
    Ok(0)
}

fn remove_wrapper(_app: Matches, command: Matches) -> Result<i32> {
    let Some(name) = wrapper_name(&command) else { return Ok(2) };
    wrappers::remove(name)?;
    Ok(0)
}

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

fn path_export(output: &mut Vec<u8>) -> Result<()> {
    let bin = jobmarshal_common::wrappers::directory()?.join("bin");
    let path = env::var_os("PATH");
    if path.as_ref().is_some_and(|path| env::split_paths(path).any(|entry| entry == bin)) {
        return Ok(());
    }
    let mut value = OsString::from(bin);
    if let Some(path) = path {
        value.push(":");
        value.push(path);
    }
    export(output, "PATH", &value);
    Ok(())
}

fn start_options(builder: CommandBuilder) -> CommandBuilder {
    builder.optopt("j", "jobs", "shared job capacity (default: available CPUs)", "N")
}

fn stats_options(builder: CommandBuilder) -> CommandBuilder {
    builder.optflag("", "zero", "reset statistics before printing them")
}

fn start(_app: Matches, command: Matches) -> Result<i32> {
    let requested = capacity::requested(command.opt_str("jobs"))?;
    let paths = Paths::discover()?;
    let mut output = Vec::new();
    export(&mut output, "JOBMARSHAL_RUNTIME_DIR", paths.directory.as_os_str());
    path_export(&mut output)?;
    lifecycle::start(&paths, requested)?;
    io::stdout().lock().write_all(&output)?;
    Ok(0)
}

fn stats(_app: Matches, command: Matches) -> Result<i32> {
    let snapshot = client::stats(&Paths::discover()?, command.opt_present("zero"))?;
    stats::print(&mut io::stdout().lock(), &snapshot)?;
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
    if let Some(status) = run::intercept() {
        return status;
    }
    Builder::new("Jobmarshal", env!("CARGO_PKG_VERSION"), env::args())
        .copyright("Copyright 2026 Julio Merino")
        .homepage(env!("CARGO_PKG_REPOSITORY"))
        .manpage("jobmarshal", "1")
        .cmd("add-wrapper", "install a per-user command wrapper", wrapper_options, add_wrapper)
        .cmd("list-wrappers", "list per-user command wrappers", no_options, list_wrappers)
        .cmd("remove-wrapper", "remove a per-user command wrapper", wrapper_options, remove_wrapper)
        .cmd("run", "run a command with the shared job budget", run::options, run::dispatch)
        .cmd("start", "start the per-user jobmarshal", start_options, start)
        .cmd("stats", "inspect in-memory jobmarshal statistics", stats_options, stats)
        .cmd("status", "inspect the per-user jobmarshal", no_options, status)
        .cmd("stop", "stop the per-user jobmarshal", no_options, stop)
        .run(command_dispatcher)
}
