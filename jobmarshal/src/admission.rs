// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Build admission and exact lease cleanup.

use crate::pool::Pool;
use anyhow::{Result, bail};
use jobmarshal_common::protocol::{Response, send};
use nix::errno::Errno;
use nix::sys::socket::{MsgFlags, recv};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub(crate) struct State {
    pub(crate) active: usize,
    pub(crate) draining: bool,
    pub(crate) pool: Pool,
    pub(crate) waiting: usize,
}

pub(crate) type Shared = Arc<Mutex<State>>;

struct Waiting<'a> {
    shared: &'a Shared,
    queued: bool,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if self.queued {
            self.shared.lock().unwrap().waiting -= 1;
        }
    }
}

struct Lease {
    shared: Shared,
    token: u8,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.shared.lock().unwrap();
        if let Err(error) = state.pool.put(self.token) {
            eprintln!("cannot return implicit job token: {error}");
        }
        state.active -= 1;
    }
}

fn disconnected(stream: &UnixStream) -> Result<bool> {
    let mut byte = [0];
    match recv(stream.as_raw_fd(), &mut byte, MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT) {
        Ok(0) => Ok(true),
        Ok(_) => bail!("unexpected data while waiting for admission"),
        Err(Errno::EAGAIN | Errno::EINTR) => Ok(false),
        Err(Errno::ECONNRESET) => Ok(true),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn acquire(stream: &mut UnixStream, shared: &Shared) -> Result<()> {
    shared.lock().unwrap().waiting += 1;
    let mut waiting = Waiting { shared, queued: true };
    let lease = loop {
        if disconnected(stream)? {
            return Ok(());
        }
        let mut state = shared.lock().unwrap();
        if state.draining {
            state.waiting -= 1;
            waiting.queued = false;
            drop(state);
            send(stream, Response::Error("jobmarshal is draining".into()))?;
            return Ok(());
        }
        if let Some(token) = state.pool.take()? {
            state.waiting -= 1;
            waiting.queued = false;
            state.active += 1;
            break Lease { shared: Arc::clone(shared), token };
        }
        drop(state);
        thread::sleep(Duration::from_millis(5));
    };
    drop(waiting);
    let fifo = shared.lock().unwrap().pool.fifo.clone();
    send(stream, Response::Granted { fifo })?;
    stream.set_read_timeout(None)?;
    let mut bytes = [0; 256];
    loop {
        match stream.read(&mut bytes) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    drop(lease);
    Ok(())
}
