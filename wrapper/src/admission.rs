// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Pool selection under lifecycle ownership and inherited build leases.

use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use jobmarshal_common::{
    client,
    descriptors::inherit,
    locks,
    protocol::{Request, Response, receive_when_ready, send},
    runtime::Paths,
};
use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) struct Allocation {
    pub(crate) fifo: PathBuf,
    _participants: File,
    _lease: UnixStream,
}

pub(crate) fn acquire(server: &Path) -> Result<Allocation> {
    let paths = Paths::discover()?;
    let result = Command::new(server)
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .with_context(|| format!("cannot start jobmarshal through {}", server.display()))?;
    ensure!(result.success(), "jobmarshal startup failed");
    let lifecycle = locks::lifecycle(&paths)?;
    let participants = locks::private_file(&paths, "participants.lock", false)?;
    FileExt::lock_shared(&participants)?;
    let mut lease = client::connect(&paths)?.context("jobmarshal stopped before admission")?;
    send(&mut lease, Request::Acquire)?;
    drop(lifecycle);
    let fifo = match receive_when_ready(&mut lease)? {
        Response::Granted { fifo } => fifo,
        Response::Error(error) => bail!("{error}"),
        response => bail!("unexpected admission response: {response:?}"),
    };
    ensure!(fifo == paths.directory.join("tokens"), "daemon granted an unexpected token FIFO");
    inherit(&participants)?;
    inherit(&lease)?;
    Ok(Allocation { fifo, _participants: participants, _lease: lease })
}
