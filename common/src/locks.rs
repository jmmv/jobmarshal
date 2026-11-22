// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Stable private lock and log files.

use crate::runtime::Paths;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use nix::fcntl::OFlag;
use nix::unistd::Uid;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::thread;
use std::time::{Duration, Instant};

/// Opens a private regular file without following a symbolic link.
pub fn private_file(paths: &Paths, name: &str, append: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .append(append)
        .mode(0o600)
        .custom_flags(OFlag::O_NOFOLLOW.bits() | OFlag::O_NONBLOCK.bits())
        .open(paths.directory.join(name))
        .with_context(|| format!("cannot open {name}"))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == Uid::current().as_raw()
            && metadata.mode() & 0o077 == 0,
        "{name} must be a private regular file owned by you"
    );
    Ok(file)
}

/// Takes an exclusive lock, bounding contention and teardown failures.
pub fn exclusive(file: &File) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                ensure!(Instant::now() < deadline, "timed out waiting for lifecycle ownership");
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// Serializes start and stop while leaving the stable lock inode in place.
pub fn lifecycle(paths: &Paths) -> Result<File> {
    let file = private_file(paths, "lifecycle.lock", false)?;
    exclusive(&file)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::private_file;
    use crate::runtime::Paths;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn test_private_file_rejects_symlink() {
        let temp = tempdir().unwrap();
        let paths = Paths::prepare(temp.path().join("runtime")).unwrap();
        let target = temp.path().join("target");
        fs::write(&target, "keep").unwrap();
        symlink(&target, paths.directory.join("daemon.log")).unwrap();
        assert!(private_file(&paths, "daemon.log", true).is_err());
        assert_eq!("keep", fs::read_to_string(target).unwrap());
    }
}
