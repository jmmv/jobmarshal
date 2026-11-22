// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Build-script activity recorder for independent Cargo workspaces.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).unwrap();
    }
}

fn lock(root: &Path) -> Lock {
    let path = root.join("lock");
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        match fs::create_dir(&path) {
            Ok(()) => return Lock(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("{error}"),
        }
    }
}

fn change(root: &Path, delta: i32) {
    let _lock = lock(root);
    let count = fs::read_to_string(root.join("count"))
        .unwrap_or_else(|_| "0".into())
        .parse::<i32>()
        .unwrap()
        + delta;
    fs::write(root.join("count"), count.to_string()).unwrap();
    let mut log = OpenOptions::new().create(true).append(true).open(root.join("events")).unwrap();
    writeln!(log, "{count}").unwrap();
}

fn main() {
    let root = PathBuf::from(env::var_os("ACTIVITY_ROOT").unwrap());
    let package = env::var("CARGO_PKG_NAME").unwrap();
    change(&root, 1);
    fs::write(root.join(format!("{package}.active")), "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(40);
    while !root.join("release").exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    fs::remove_file(root.join(format!("{package}.active"))).unwrap();
    change(&root, -1);
}
