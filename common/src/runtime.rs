// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Private per-user rendezvous directories.

use anyhow::{Context, Result, ensure};
use nix::unistd::Uid;
use std::env;
use std::fs::{self, DirBuilder, Metadata};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

/// A validated runtime directory.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Directory containing the socket, log, and stable locks.
    pub directory: PathBuf,
}

impl Paths {
    /// Discovers a runtime directory using the application override or XDG API.
    pub fn discover() -> Result<Self> {
        let directory = match env::var_os("JOBMARSHAL_RUNTIME_DIR") {
            Some(directory) => PathBuf::from(directory),
            None => runtime_directory(
                xdg::BaseDirectories::new().get_runtime_directory().ok().map(PathBuf::as_path),
                Uid::current().as_raw(),
            ),
        };
        Self::prepare(directory)
    }

    /// Creates and validates a chosen final path, regardless of discovery method.
    pub fn prepare(directory: PathBuf) -> Result<Self> {
        ensure!(directory.is_absolute(), "runtime directory must be absolute");
        // Strip trailing separators and dots before lstat, which otherwise follows a final link.
        let directory: PathBuf = directory.components().collect();
        let text = directory.to_str().context("runtime directory must be UTF-8")?;
        ensure!(
            !text.chars().any(char::is_whitespace),
            "runtime directory cannot contain whitespace"
        );
        match DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error).context("cannot create runtime directory"),
        }
        validate(&directory)?;
        Ok(Self { directory })
    }

    /// Returns the control socket path.
    pub fn socket(&self) -> PathBuf {
        self.directory.join("control.sock")
    }
}

fn validate(directory: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(directory)?;
    validate_metadata(&metadata, Uid::current().as_raw())
        .with_context(|| format!("unsafe runtime directory: {}", directory.display()))
}

fn runtime_directory(base: Option<&Path>, uid: u32) -> PathBuf {
    base.map(|path| path.join("jobmarshal"))
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/jobmarshal-{uid}")))
}

fn validate_metadata(metadata: &Metadata, uid: u32) -> Result<()> {
    ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o077 == 0,
        "runtime directory must be a real directory owned by you with mode 0700"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Paths, runtime_directory, validate_metadata};
    use std::ffi::OsString;
    use std::fs::{self, Permissions};
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_runtime_directory_missing_xdg_fallback() {
        assert_eq!(PathBuf::from("/tmp/jobmarshal-1234"), runtime_directory(None, 1234));
        assert_eq!(
            PathBuf::from("/private/runtime/jobmarshal"),
            runtime_directory(Some(std::path::Path::new("/private/runtime")), 1234)
        );
    }

    #[test]
    fn test_runtime_directory_wrong_owner() {
        let temp = tempdir().unwrap();
        let metadata = fs::metadata(temp.path()).unwrap();
        assert!(validate_metadata(&metadata, metadata.uid().wrapping_add(1)).is_err());
    }

    #[test]
    fn test_prepare_safe() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("runtime");
        assert_eq!(path, Paths::prepare(path.clone()).unwrap().directory);
        assert_eq!(0o700, fs::metadata(path).unwrap().permissions().mode() & 0o777);
    }

    #[test]
    fn test_prepare_unsafe() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("unsafe");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o755)).unwrap();
        assert!(Paths::prepare(path.clone()).is_err());
        fs::remove_dir(&path).unwrap();
        symlink(temp.path(), &path).unwrap();
        assert!(Paths::prepare(path.clone()).is_err());
        assert!(Paths::prepare(path.join("")).is_err());
        assert!(Paths::prepare(path.join(".")).is_err());
        fs::remove_file(&path).unwrap();
        fs::write(&path, "").unwrap();
        assert!(Paths::prepare(path).is_err());
    }

    #[test]
    fn test_prepare_invalid_path() {
        for path in [
            PathBuf::from("relative"),
            PathBuf::from("/tmp/has space"),
            PathBuf::from(OsString::from_vec(b"/tmp/invalid-\xff".to_vec())),
        ] {
            assert!(Paths::prepare(path).is_err());
        }
    }
}
