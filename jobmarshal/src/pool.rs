// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Native FIFO tokens, opened independently from clients' file descriptions.

use anyhow::{Context, Result, ensure};
use jobmarshal_common::runtime::Paths;
use nix::fcntl::OFlag;
use nix::libc;
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

pub(crate) struct Pool {
    file: File,
    pub(crate) fifo: PathBuf,
    pub(crate) jobs: usize,
}

impl Pool {
    #[allow(unsafe_code)]
    pub(crate) fn available(&self) -> Result<usize> {
        let mut bytes: libc::c_int = 0;
        // SAFETY: FIONREAD on Linux and FreeBSD writes one c_int through a valid pointer.
        // It observes queued FIFO bytes without consuming tokens or changing descriptor flags.
        if unsafe { libc::ioctl(self.file.as_raw_fd(), libc::FIONREAD, &mut bytes) } == -1 {
            return Err(io::Error::last_os_error()).context("cannot observe available FIFO tokens");
        }
        bytes.try_into().context("FIFO reported a negative token count")
    }

    pub(crate) fn create(paths: &Paths, jobs: usize) -> Result<Self> {
        let fifo = paths.directory.join("tokens");
        match fs::symlink_metadata(&fifo) {
            Ok(metadata) => {
                ensure!(
                    metadata.file_type().is_fifo()
                        && metadata.uid() == fs::metadata(&paths.directory)?.uid(),
                    "refusing to replace an unrelated token FIFO path"
                );
                fs::remove_file(&fifo)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK).bits())
            .open(&fifo)?;
        for _ in 0..jobs {
            if let Err(error) = file.write_all(b"+") {
                drop(file);
                fs::remove_file(&fifo)?;
                return Err(error).context("requested jobs exceed FIFO capacity");
            }
        }
        Ok(Self { file, fifo, jobs })
    }

    pub(crate) fn take(&mut self) -> io::Result<Option<u8>> {
        let mut token = [0];
        match self.file.read(&mut token) {
            Ok(1) => Ok(Some(token[0])),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
            _ => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "token FIFO closed")),
        }
    }

    pub(crate) fn put(&mut self, token: u8) -> io::Result<()> {
        self.file.write_all(&[token])
    }
}
