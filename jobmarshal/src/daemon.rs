// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Control service and synchronized teardown.

use crate::admission::{self, Shared, State};
use crate::pool::Pool;
use anyhow::{Context, Result, ensure};
use jobmarshal_common::protocol::{Request, Response, receive, send};
use jobmarshal_common::runtime::Paths;
use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
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

fn handle(mut stream: UnixStream, shared: Shared) -> Result<()> {
    let request = match receive(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            return send(&mut stream, Response::Error(format!("invalid control request: {error}")));
        }
    };
    let response = match request {
        Request::Acquire => return admission::acquire(&mut stream, &shared),
        Request::Status => {
            let state = shared.lock().unwrap();
            Response::Status {
                active: state.active,
                draining: state.draining,
                jobs: state.pool.jobs,
                pid: std::process::id(),
                waiting: state.waiting,
            }
        }
        Request::Stop => {
            shared.lock().unwrap().draining = true;
            loop {
                let state = shared.lock().unwrap();
                if state.active == 0 && state.waiting == 0 {
                    break;
                }
                drop(state);
                thread::sleep(Duration::from_millis(5));
            }
            Response::Stopped
        }
    };
    send(&mut stream, response)
}

pub(crate) fn run(paths: &Paths, listener: UnixListener, _owner: File, pool: Pool) -> Result<()> {
    let shared = Arc::new(Mutex::new(State { active: 0, draining: false, pool, waiting: 0 }));
    let mut handlers = Vec::new();
    let outcome = loop {
        let state = shared.lock().unwrap();
        if state.draining && state.active == 0 && state.waiting == 0 {
            break Ok(());
        }
        drop(state);
        match listener.accept() {
            Ok((stream, _)) => {
                let shared = Arc::clone(&shared);
                handlers.push(thread::spawn(move || {
                    if let Err(error) = handle(stream, shared) {
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
    fs::remove_file(&shared.lock().unwrap().pool.fifo).context("cannot remove token FIFO")?;
    outcome
}
