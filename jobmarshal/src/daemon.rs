// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Control service and synchronized teardown.

use anyhow::{Context, Result, ensure};
use jobmarshal_common::protocol::{Request, Response, receive, send};
use jobmarshal_common::runtime::Paths;
use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

pub(crate) fn listen(paths: &Paths) -> Result<UnixListener> {
    match fs::symlink_metadata(paths.socket()) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_socket()
                    && metadata.uid() == fs::metadata(&paths.directory)?.uid(),
                "refusing to replace an unrelated control socket path"
            );
            fs::remove_file(paths.socket()).context("cannot remove stale control socket")?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("cannot remove stale control socket"),
    }
    let listener = UnixListener::bind(paths.socket()).context("cannot bind control socket")?;
    fs::set_permissions(paths.socket(), Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

fn handle(mut stream: UnixStream, jobs: usize, draining: &AtomicBool) -> Result<()> {
    let request = match receive(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            return send(&mut stream, Response::Error(format!("invalid control request: {error}")));
        }
    };
    let response = match request {
        Request::Status => Response::Status {
            active: 0,
            draining: draining.load(Ordering::SeqCst),
            jobs,
            pid: std::process::id(),
            waiting: 0,
        },
        Request::Stop => {
            draining.store(true, Ordering::SeqCst);
            Response::Stopped
        }
    };
    send(&mut stream, response)
}

pub(crate) fn run(paths: &Paths, listener: UnixListener, _owner: File, jobs: usize) -> Result<()> {
    let draining = Arc::new(AtomicBool::new(false));
    let mut handlers = Vec::new();
    let outcome = loop {
        if draining.load(Ordering::SeqCst) {
            break Ok(());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let draining = draining.clone();
                handlers.push(thread::spawn(move || {
                    if let Err(error) = handle(stream, jobs, &draining) {
                        eprintln!("control request: {error:#}");
                    }
                }));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => break Err(error).context("cannot accept control request"),
        }
        let mut index = 0;
        while index < handlers.len() {
            if handlers[index].is_finished() {
                let _ = handlers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
    };
    drop(listener);
    for handler in handlers {
        let _ = handler.join();
    }
    fs::remove_file(paths.socket()).context("cannot remove control socket")?;
    outcome
}
