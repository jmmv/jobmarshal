// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Ordinary control client operations.

use crate::protocol::{Request, Response, TIMEOUT, receive, send};
use crate::runtime::Paths;
use crate::stats::Stats;
use anyhow::{Context, Result, bail, ensure};
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, poll};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType, UnixAddr, sockopt};
use std::io;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};

fn connect_bounded(paths: &Paths) -> io::Result<UnixStream> {
    let fd = socket::socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )?;
    let address = UnixAddr::new(&paths.socket())?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match socket::connect(fd.as_raw_fd(), &address) {
            Ok(()) | Err(Errno::EISCONN) => break,
            Err(Errno::EAGAIN) => {
                // Linux does not queue a nonblocking connection when the Unix backlog is full.
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "control connection timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(Errno::EINPROGRESS | Errno::EALREADY) => {
                let timeout = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u16::MAX as u128) as u16;
                let mut descriptors = [PollFd::new(fd.as_fd(), PollFlags::POLLOUT)];
                if poll(&mut descriptors, timeout)? == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "control connection timed out",
                    ));
                }
                let error = socket::getsockopt(&fd, sockopt::SocketError)?;
                if error != 0 {
                    return Err(io::Error::from_raw_os_error(error));
                }
                break;
            }
            Err(Errno::EINTR) => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "control connection timed out",
                    ));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    Ok(stream)
}

/// Connects within a deadline; only missing/refused sockets mean absence.
pub fn connect(paths: &Paths) -> Result<Option<UnixStream>> {
    match connect_bounded(paths) {
        Ok(stream) => Ok(Some(stream)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error).context("cannot connect to jobmarshal"),
    }
}

/// Requests statistics without starting a daemon.
pub fn stats(paths: &Paths, zero: bool) -> Result<Stats> {
    let mut stream = connect(paths)?.context("jobmarshal is not running")?;
    send(&mut stream, Request::Stats { zero })?;
    match receive(&mut stream)? {
        Response::Stats(stats) => Ok(stats),
        Response::Error(error) => bail!("{error}"),
        response => bail!("unexpected stats response: {response:?}"),
    }
}

/// Requests state without starting a daemon.
pub fn status(paths: &Paths) -> Result<Option<Response>> {
    let Some(mut stream) = connect(paths)? else { return Ok(None) };
    send(&mut stream, Request::Status)?;
    match receive(&mut stream)? {
        response @ Response::Status { jobs, .. } => {
            ensure!(jobs > 0, "daemon reported zero capacity");
            Ok(Some(response))
        }
        Response::Error(error) => bail!("{error}"),
        response => bail!("unexpected status response: {response:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::connect_bounded;
    use crate::protocol::TIMEOUT;
    use crate::runtime::Paths;
    use nix::sys::socket::{Backlog, listen};
    use std::os::unix::net::UnixListener;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    #[test]
    fn test_connect_full_backlog_is_bounded() {
        let temp = tempdir().unwrap();
        let paths = Paths::prepare(temp.path().join("runtime")).unwrap();
        let listener = UnixListener::bind(paths.socket()).unwrap();
        listen(&listener, Backlog::new(1).unwrap()).unwrap();
        let mut connections = Vec::new();
        for _ in 0..16 {
            let start = Instant::now();
            match connect_bounded(&paths) {
                Ok(stream) => connections.push(stream),
                Err(_) => {
                    assert!(start.elapsed() < TIMEOUT + Duration::from_secs(1));
                    return;
                }
            }
        }
        panic!("listener backlog did not fill");
    }
}
