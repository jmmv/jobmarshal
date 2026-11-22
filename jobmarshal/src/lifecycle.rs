// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Daemon startup and shutdown under stable lifecycle ownership.

use crate::{daemon, pool::Pool, wrappers};
use anyhow::{Context, Result, ensure};
use daemonize::{Daemonize, Outcome};
use fs2::FileExt;
use jobmarshal_common::{
    client, locks,
    protocol::{Request, Response, receive_when_ready, send},
    runtime::Paths,
};
use std::thread;
use std::time::{Duration, Instant};

fn check_existing(response: Response, requested: Option<usize>) -> Result<()> {
    let Response::Status { draining, jobs, .. } = response else {
        anyhow::bail!("unexpected daemon status")
    };
    ensure!(!draining, "jobmarshal is draining; wait for stop to finish");
    ensure!(
        requested.is_none_or(|requested| requested == jobs),
        "running jobmarshal has {jobs} jobs; stop it before changing capacity"
    );
    Ok(())
}

pub(crate) fn start(paths: &Paths, requested: Option<usize>) -> Result<()> {
    let lifecycle = locks::lifecycle(paths)?;
    if let Some(response) = client::status(paths)? {
        return check_existing(response, requested);
    }
    let participants = locks::private_file(paths, "participants.lock", false)?;
    participants.try_lock_exclusive().context(
        "previous builds are still running; let them finish before restarting the jobmarshal",
    )?;
    let owner = locks::private_file(paths, "daemon.lock", false)?;
    owner.try_lock_exclusive().context("daemon still owns its resources; retry after shutdown")?;
    let jobs =
        requested.map(Ok).unwrap_or_else(|| thread::available_parallelism().map(usize::from))?;
    wrappers::refresh().context("cannot refresh managed wrappers")?;
    let pool = Pool::create(paths, jobs)?;
    let listener = daemon::listen(paths)?;
    let log = locks::private_file(paths, "daemon.log", true)?;
    match Daemonize::new()
        .working_directory(&paths.directory)
        .umask(0o077)
        .stdout(log.try_clone()?)
        .stderr(log)
        .execute()
    {
        Outcome::Child(result) => {
            // flock ownership follows the open file description across fork.  Drop this copy
            // rather than unlocking it, so the parent keeps serializing startup readiness.
            drop(lifecycle);
            drop(participants);
            let outcome = result
                .context("cannot daemonize jobmarshal")
                .and_then(|_| daemon::run(paths, listener, owner, pool));
            if let Err(error) = &outcome {
                eprintln!("jobmarshal: {error:#}");
            }
            std::process::exit(if outcome.is_ok() { 0 } else { 1 });
        }
        Outcome::Parent(result) => {
            let parent = result.context("cannot daemonize jobmarshal")?;
            ensure!(parent.first_child_exit_code == 0, "daemonization failed; inspect daemon.log");
            drop(listener);
            drop(owner);
            drop(participants);
            drop(pool);
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(response) = client::status(paths)? {
                    check_existing(response, requested)?;
                    return Ok(());
                }
                ensure!(
                    Instant::now() < deadline,
                    "daemon did not become ready; inspect {}",
                    paths.directory.join("daemon.log").display()
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub(crate) fn stop(paths: &Paths) -> Result<()> {
    let _lifecycle = locks::lifecycle(paths)?;
    let mut stream = client::connect(paths)?.context("jobmarshal is not running")?;
    send(&mut stream, Request::Stop)?;
    ensure!(
        matches!(receive_when_ready(&mut stream)?, Response::Stopped),
        "unexpected stop response"
    );
    let owner = locks::private_file(paths, "daemon.lock", false)?;
    locks::exclusive(&owner).context("daemon did not finish shutdown")?;
    ensure!(!paths.socket().exists(), "daemon left its control socket behind");
    Ok(())
}
