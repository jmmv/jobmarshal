// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Versioned newline-delimited control frames.

use anyhow::{Result, ensure};
use nix::poll::{PollFd, PollFlags, poll};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Maximum frame size, including its final newline.
pub const MAX_FRAME: usize = 8192;
/// Ordinary control-operation deadline.
pub const TIMEOUT: Duration = Duration::from_secs(3);

/// A daemon control request.
#[derive(Debug, Deserialize, Serialize)]
pub enum Request {
    /// Reserve one implicit job slot and keep the connection as its lease.
    Acquire,
    /// Inspect the responsive daemon.
    Status,
    /// Stop the daemon and acknowledge before teardown.
    Stop,
}

/// A daemon control response.
#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    /// A rejected operation or malformed request.
    Error(String),
    /// An implicit slot is reserved until this connection closes.
    Granted {
        /// Native GNU jobserver FIFO authentication path.
        fifo: PathBuf,
    },
    /// Current service state.
    Status {
        /// Number of admitted builds.
        active: usize,
        /// Whether the daemon is shutting down.
        draining: bool,
        /// Configured maximum parallelism.
        jobs: usize,
        /// Process identifier.
        pid: u32,
        /// Number of queued builds.
        waiting: usize,
    },
    /// Stop was accepted; teardown follows completion of all responses.
    Stopped,
}

#[derive(Deserialize, Serialize)]
struct Frame<T> {
    version: u32,
    body: T,
}

/// Sends a bounded frame with a total write deadline.
pub fn send<T: Serialize>(stream: &mut UnixStream, body: T) -> Result<()> {
    let mut bytes = serde_json::to_vec(&Frame { version: 1, body })?;
    bytes.push(b'\n');
    ensure!(bytes.len() <= MAX_FRAME, "control message exceeds 8 KiB");
    let deadline = Instant::now() + TIMEOUT;
    let mut remaining = bytes.as_slice();
    while !remaining.is_empty() {
        stream.set_write_timeout(Some(
            deadline.saturating_duration_since(Instant::now()).max(Duration::from_nanos(1)),
        ))?;
        let count = stream.write(remaining)?;
        ensure!(count > 0, "control connection closed while writing");
        remaining = &remaining[count..];
        ensure!(Instant::now() <= deadline, "control write timed out");
    }
    Ok(())
}

/// Receives one frame without buffering bytes belonging to a subsequent frame.
pub fn receive<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T> {
    let deadline = Instant::now() + TIMEOUT;
    let mut bytes = Vec::new();
    loop {
        ensure!(Instant::now() < deadline, "control read timed out");
        stream.set_read_timeout(Some(
            deadline.saturating_duration_since(Instant::now()).max(Duration::from_nanos(1)),
        ))?;
        let mut byte = [0];
        ensure!(stream.read(&mut byte)? == 1, "incomplete control frame");
        bytes.push(byte[0]);
        ensure!(bytes.len() <= MAX_FRAME, "control message exceeds 8 KiB");
        if byte[0] == b'\n' {
            break;
        }
    }
    let frame: Frame<T> = serde_json::from_slice(&bytes)?;
    ensure!(frame.version == 1, "unsupported jobmarshal protocol version {}", frame.version);
    Ok(frame.body)
}

/// Waits for admission or draining to finish, then reads a bounded response frame.
pub fn receive_when_ready<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T> {
    let mut descriptors = [PollFd::new(stream.as_fd(), PollFlags::POLLIN)];
    loop {
        match poll(&mut descriptors, None::<u16>) {
            Ok(_) => return receive(stream),
            Err(nix::errno::Errno::EINTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, receive, send};
    use std::io::Write;
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;

    #[test]
    fn test_protocol_roundtrip() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        send(&mut client, Request::Status).unwrap();
        send(&mut client, Request::Stop).unwrap();
        assert!(matches!(receive::<Request>(&mut server).unwrap(), Request::Status));
        assert!(matches!(receive::<Request>(&mut server).unwrap(), Request::Stop));
        send(&mut server, Response::Stopped).unwrap();
        assert!(matches!(receive::<Response>(&mut client).unwrap(), Response::Stopped));
    }

    #[test]
    fn test_protocol_invalid_frames() {
        for bytes in [
            b"{\"version\":2,\"body\":\"Status\"}\n".to_vec(),
            b"invalid\n".to_vec(),
            b"{}".to_vec(),
            vec![b'x'; 8193],
        ] {
            let (mut client, mut server) = UnixStream::pair().unwrap();
            client.write_all(&bytes).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            assert!(receive::<Request>(&mut server).is_err());
        }
    }
}
