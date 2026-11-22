// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Atomic installation and refresh of user-owned wrappers.

use anyhow::{Context, Result, ensure};
use jobmarshal_common::{locks, runtime::Paths, wrappers};
use nix::unistd::Uid;
use std::env;
use std::fs::{self, DirBuilder, File, Metadata};
use std::io::{self, BufRead, BufReader};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use tempfile::tempdir_in;

pub(crate) fn installed() -> Result<PathBuf> {
    let executable = env::current_exe().context("cannot locate jobmarshal executable")?;
    let directory = executable.parent().context("jobmarshal executable has no parent directory")?;
    Ok(directory.join("../libexec/jobmarshal-wrapper"))
}

fn metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
    }
}

fn directory(path: &Path, create: bool) -> Result<bool> {
    if create {
        DirBuilder::new().recursive(true).mode(0o700).create(path)?;
    }
    let Some(metadata) = metadata(path)? else { return Ok(false) };
    ensure!(
        metadata.is_dir()
            && metadata.uid() == Uid::current().as_raw()
            && metadata.mode() & 0o022 == 0,
        "{} must be a real directory owned by you without group/other write access",
        path.display()
    );
    Ok(true)
}

struct Store {
    root: PathBuf,
    _lock: File,
}

impl Store {
    fn open(create: bool) -> Result<Option<Self>> {
        let root = wrappers::directory()?;
        if !directory(&root, create)? {
            return Ok(None);
        }
        let lock = locks::private_file(&Paths { directory: root.clone() }, "wrappers.lock", false)?;
        locks::exclusive(&lock).context("cannot lock managed wrappers")?;
        Ok(Some(Self { root, _lock: lock }))
    }

    fn names(&self) -> Result<Vec<String>> {
        let bin = self.root.join("bin");
        if !directory(&bin, false)? {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&bin)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().context("managed wrapper name must be ASCII")?;
            ensure!(
                wrappers::valid_name(name) && entry.file_type()?.is_file(),
                "unexpected managed wrapper entry: {}",
                entry.path().display()
            );
            names.push(name.to_owned());
        }
        names.sort();
        Ok(names)
    }

    fn canonical(&self) -> Result<PathBuf> {
        let libexec = self.root.join("libexec");
        directory(&libexec, true)?;
        let destination = libexec.join("jobmarshal-wrapper");
        install(&installed()?, &destination, false)?;
        Ok(destination)
    }

    fn alias(&self, source: &Path, name: &str) -> Result<()> {
        let bin = self.root.join("bin");
        directory(&bin, true)?;
        install(source, &bin.join(name), true)
    }
}

fn same_contents(source: &Path, destination: &Path) -> Result<bool> {
    let mut source = BufReader::new(File::open(source)?);
    let mut destination = BufReader::new(File::open(destination)?);
    loop {
        let left = source.fill_buf()?;
        let right = destination.fill_buf()?;
        let length = left.len().min(right.len());
        if length == 0 {
            return Ok(left.is_empty() && right.is_empty());
        }
        if left[..length] != right[..length] {
            return Ok(false);
        }
        source.consume(length);
        destination.consume(length);
    }
}

fn install(source: &Path, destination: &Path, link: bool) -> Result<()> {
    install_with(source, destination, link, |source, destination| {
        fs::hard_link(source, destination)
    })
    .with_context(|| format!("cannot install wrapper {}", destination.display()))
}

fn install_with(
    source: &Path,
    destination: &Path,
    link: bool,
    hard_link: impl Fn(&Path, &Path) -> io::Result<()>,
) -> Result<()> {
    let source_metadata = fs::metadata(source)
        .with_context(|| format!("cannot inspect source wrapper {}", source.display()))?;
    ensure!(
        source_metadata.is_file() && source_metadata.mode() & 0o111 != 0,
        "source wrapper must be an executable regular file: {}",
        source.display()
    );
    let existing = metadata(destination)?;
    if let Some(existing) = &existing {
        ensure!(existing.is_file(), "wrapper destination must be a regular file");
        if link
            && existing.dev() == source_metadata.dev()
            && existing.ino() == source_metadata.ino()
        {
            return Ok(());
        }
    }
    let parent = destination.parent().context("wrapper destination has no parent")?;
    let staging = if link { parent.parent().context("wrapper bin has no parent")? } else { parent };
    let work = tempdir_in(staging)?;
    let temporary = work.path().join("wrapper");
    if !link || hard_link(source, &temporary).is_err() {
        if let Some(existing) = &existing
            && existing.mode() & 0o111 == source_metadata.mode() & 0o111
            && existing.len() == source_metadata.len()
            && same_contents(source, destination)?
        {
            return Ok(());
        }
        fs::copy(source, &temporary)?;
    }
    fs::rename(&temporary, destination)?;
    Ok(())
}

pub(crate) fn add(name: &str) -> Result<()> {
    let store = Store::open(true)?.unwrap();
    store.names()?;
    let canonical = store.canonical()?;
    store.alias(&canonical, name)
}

pub(crate) fn list() -> Result<Vec<String>> {
    match Store::open(false)? {
        Some(store) => store.names(),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn remove(name: &str) -> Result<()> {
    if let Some(store) = Store::open(false)?
        && store.names()?.iter().any(|entry| entry == name)
    {
        fs::remove_file(store.root.join("bin").join(name))?;
    }
    Ok(())
}

pub(crate) fn refresh() -> Result<()> {
    if let Some(store) = Store::open(false)? {
        let names = store.names()?;
        if !names.is_empty() {
            let canonical = store.canonical()?;
            for name in names {
                store.alias(&canonical, &name)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{install, install_with};
    use std::fs::{self, Permissions};
    use std::io::{self, Read};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tempfile::{tempdir, tempdir_in};

    #[test]
    fn test_install_copy_fallback_and_refresh() {
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, "first").unwrap();
        fs::set_permissions(&source, Permissions::from_mode(0o755)).unwrap();
        let copy = || {
            install_with(&source, &destination, true, |_, _| {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            })
            .unwrap();
        };
        copy();
        let inode = fs::metadata(&destination).unwrap().ino();
        assert_ne!(fs::metadata(&source).unwrap().ino(), inode);
        copy();
        assert_eq!(inode, fs::metadata(&destination).unwrap().ino());
        let mut previous = fs::File::open(&destination).unwrap();
        fs::write(&source, "other").unwrap();
        copy();
        let mut contents = String::new();
        previous.read_to_string(&mut contents).unwrap();
        assert_eq!("first", contents);
        assert_eq!(b"other", fs::read(&destination).unwrap().as_slice());
        assert_ne!(inode, fs::metadata(&destination).unwrap().ino());
    }

    #[test]
    fn test_install_cross_filesystem_fallback() {
        let root = tempdir().unwrap();
        let Ok(other) = tempdir_in("/dev/shm") else { return };
        if fs::metadata(root.path()).unwrap().dev() == fs::metadata(other.path()).unwrap().dev() {
            return;
        }
        let source = root.path().join("source");
        let destination = other.path().join("wrapper");
        fs::write(&source, b"executable").unwrap();
        fs::set_permissions(&source, Permissions::from_mode(0o755)).unwrap();
        install(&source, &destination, true).unwrap();
        assert_eq!(b"executable", fs::read(&destination).unwrap().as_slice());
        let identity = fs::metadata(&destination).unwrap().ino();
        install(&source, &destination, true).unwrap();
        assert_eq!(identity, fs::metadata(&destination).unwrap().ino());
    }
}
