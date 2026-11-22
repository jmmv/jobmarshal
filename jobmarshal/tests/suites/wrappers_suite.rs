// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Persistent aliases, upgrades, and shell initialization.

use crate::common::service::{Service, copy_executable};
use assert_cmd::Command;
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::thread;

fn install(service: &Service) -> PathBuf {
    let root = service.root.path().join("installation with spaces");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("libexec")).unwrap();
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_jobmarshal"));
    copy_executable(&binary, root.join("bin/jobmarshal"));
    copy_executable(
        binary.with_file_name("jobmarshal-wrapper"),
        root.join("libexec/jobmarshal-wrapper"),
    );
    root
}

fn command(service: &Service) -> Command {
    let mut command =
        Command::new(service.root.path().join("installation with spaces/bin/jobmarshal"));
    command
        .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
        .env("XDG_DATA_HOME", service.root.path().join("data"))
        .env("JOBMARSHAL_JOBS", "1")
        .timeout(std::time::Duration::from_secs(25));
    command
}

fn data(service: &Service) -> PathBuf {
    service.root.path().join("data/jobmarshal")
}

fn inode(path: impl AsRef<std::path::Path>) -> (u64, u64) {
    let metadata = fs::metadata(path).unwrap();
    (metadata.dev(), metadata.ino())
}

#[test]
fn test_management_and_validation() {
    let service = Service::new();
    let root = install(&service);
    command(&service).arg("list-wrappers").assert().success().stdout("").stderr("");
    assert!(!data(&service).exists());
    for name in ["cargo", "cc", "cargo", "c++", "a.b_1-2"] {
        command(&service).args(["add-wrapper", name]).assert().success().stdout("").stderr("");
        let canonical = data(&service).join("libexec/jobmarshal-wrapper");
        assert_eq!(inode(&canonical), inode(data(&service).join("bin").join(name)));
        assert_ne!(inode(&canonical), inode(root.join("libexec/jobmarshal-wrapper")));
    }
    command(&service).arg("list-wrappers").assert().success().stdout("a.b_1-2\nc++\ncargo\ncc\n");
    for name in ["", "../cc", "/cc", ".cc", "cc x", "jobmarshal", "jobmarshal-wrapper", "café"] {
        command(&service).args(["add-wrapper", name]).assert().code(2).stdout("");
        command(&service).args(["remove-wrapper", name]).assert().code(2).stdout("");
    }
    for action in ["add-wrapper", "remove-wrapper"] {
        command(&service).arg(action).assert().code(2).stdout("");
        command(&service).args([action, "cc", "extra"]).assert().code(2).stdout("");
    }
    for _ in 0..2 {
        command(&service).args(["remove-wrapper", "cargo"]).assert().success().stdout("");
    }
    command(&service).arg("list-wrappers").assert().success().stdout("a.b_1-2\nc++\ncc\n");
    assert!(!service.runtime.exists());
}

#[test]
fn test_refresh_only_for_new_daemon_and_retry() {
    let service = Service::new();
    let root = install(&service);
    command(&service).args(["add-wrapper", "tool"]).assert().success();
    let canonical = data(&service).join("libexec/jobmarshal-wrapper");
    let alias = data(&service).join("bin/tool");
    let original = inode(&canonical);
    command(&service).arg("start").assert().success();
    assert_eq!(original, inode(&canonical));
    let source = root.join("libexec/jobmarshal-wrapper");
    OpenOptions::new().append(true).open(&source).unwrap().write_all(b"upgrade").unwrap();
    command(&service).arg("start").assert().success();
    assert_eq!(original, inode(&canonical));
    command(&service).arg("stop").assert().success();
    fs::rename(&source, root.join("saved-wrapper")).unwrap();
    command(&service).arg("start").assert().code(1).stdout("");
    assert_eq!(original, inode(&canonical));
    assert_eq!(original, inode(&alias));
    assert!(!service.runtime.join("control.sock").exists());
    fs::rename(root.join("saved-wrapper"), &source).unwrap();
    command(&service).arg("start").assert().success();
    assert_ne!(original, inode(&canonical));
    assert_eq!(inode(&canonical), inode(&alias));
    assert_eq!(fs::read(&source).unwrap(), fs::read(&canonical).unwrap());
    command(&service).arg("stop").assert().success();
    let upgraded = inode(&canonical);
    command(&service).arg("start").assert().success();
    assert_eq!(upgraded, inode(&canonical));
    assert_eq!(upgraded, inode(&alias));
}

#[test]
fn test_unexpected_entries_and_unsafe_paths() {
    let service = Service::new();
    install(&service);
    command(&service).args(["add-wrapper", "cc"]).assert().success();
    let bin = data(&service).join("bin");
    let target = service.root.path().join("keep");
    fs::write(&target, "keep").unwrap();
    symlink(&target, bin.join("bad")).unwrap();
    for args in [
        vec!["list-wrappers"],
        vec!["add-wrapper", "cargo"],
        vec!["remove-wrapper", "cc"],
        vec!["start"],
    ] {
        command(&service).args(args).assert().code(1).stdout("");
    }
    assert_eq!(b"keep", fs::read(&target).unwrap().as_slice());
    fs::remove_file(bin.join("bad")).unwrap();
    fs::create_dir(bin.join("bad")).unwrap();
    command(&service).arg("list-wrappers").assert().code(1);
    fs::remove_dir(bin.join("bad")).unwrap();
    fs::set_permissions(&bin, Permissions::from_mode(0o777)).unwrap();
    command(&service).arg("list-wrappers").assert().code(1);
    fs::set_permissions(&bin, Permissions::from_mode(0o700)).unwrap();
    fs::rename(&bin, data(&service).join("saved-bin")).unwrap();
    symlink(data(&service).join("saved-bin"), &bin).unwrap();
    command(&service).args(["add-wrapper", "cargo"]).assert().code(1);
    fs::remove_file(&bin).unwrap();
    fs::rename(data(&service).join("saved-bin"), &bin).unwrap();
    let canonical = data(&service).join("libexec/jobmarshal-wrapper");
    fs::remove_file(&canonical).unwrap();
    symlink(&target, &canonical).unwrap();
    command(&service).args(["add-wrapper", "cargo"]).assert().code(1);
    command(&service)
        .env("XDG_DATA_HOME", service.root.path().join("bad:path"))
        .arg("start")
        .assert()
        .code(1)
        .stdout("");
}

#[test]
fn test_concurrent_management() {
    let service = Service::new();
    install(&service);
    let workers: Vec<_> = (0..8)
        .map(|index| {
            let mut command = command(&service);
            thread::spawn(move || {
                command.args(["add-wrapper", &format!("tool{index}")]).assert().success();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    command(&service)
        .arg("list-wrappers")
        .assert()
        .success()
        .stdout("tool0\ntool1\ntool2\ntool3\ntool4\ntool5\ntool6\ntool7\n");
}

#[test]
fn test_managed_alias_autostart_and_copy() {
    let service = Service::new();
    let root = install(&service);
    command(&service).args(["add-wrapper", "sh"]).assert().success();
    let bin = data(&service).join("bin");
    // Both managed and configure-style aliases may appear before the real command.
    fs::hard_link(root.join("libexec/jobmarshal-wrapper"), root.join("bin/sh")).unwrap();
    let path = env::join_paths([bin.clone(), root.join("bin"), PathBuf::from("/bin")]).unwrap();
    for copy in [false, true] {
        if copy {
            fs::remove_file(bin.join("sh")).unwrap();
            // Copy in a child so concurrent test forks cannot inherit a writable executable FD.
            Command::new("cp")
                .arg(data(&service).join("libexec/jobmarshal-wrapper"))
                .arg(bin.join("sh"))
                .assert()
                .success();
        }
        let mut shell = Command::new(bin.join("sh"));
        shell
            .env("PATH", &path)
            .env("XDG_DATA_HOME", service.root.path().join("data"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .env("JOBMARSHAL_JOBS", "1")
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS")
            .env_remove("MFLAGS")
            .timeout(std::time::Duration::from_secs(25))
            .args(["-c", "sh -c 'test -n \"$MAKEFLAGS\" && exit 23'"])
            .assert()
            .code(23);
        assert!(service.status().contains("jobs=1"));
        command(&service).arg("stop").assert().success();
    }
}

#[test]
fn test_managed_alias_explicit_path_without_path_entry() {
    let service = Service::new();
    let root = install(&service);
    command(&service).args(["add-wrapper", "sh"]).assert().success();
    let alias = data(&service).join("bin/sh");
    let path = env::join_paths([root.join("bin"), PathBuf::from("/bin")]).unwrap();
    for copy in [false, true] {
        if copy {
            fs::remove_file(&alias).unwrap();
            Command::new("cp")
                .arg(data(&service).join("libexec/jobmarshal-wrapper"))
                .arg(&alias)
                .assert()
                .success();
        }
        for invocation in [
            &alias,
            &PathBuf::from("data/jobmarshal/bin/sh"),
            &PathBuf::from("./data/jobmarshal/bin/sh"),
        ] {
            Command::new(invocation)
                .current_dir(service.root.path())
                .env("PATH", &path)
                .env("XDG_DATA_HOME", service.root.path().join("data"))
                .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
                .env("JOBMARSHAL_JOBS", "1")
                .env_remove("CARGO_MAKEFLAGS")
                .env_remove("MAKEFLAGS")
                .env_remove("MFLAGS")
                .timeout(std::time::Duration::from_secs(25))
                .args(["-c", "test -n \"$MAKEFLAGS\" && exit 23"])
                .assert()
                .code(23);
            assert!(service.status().contains("jobs=1"));
            command(&service).arg("stop").assert().success();
        }
    }
}

#[test]
fn test_path_exports_bytes_quoting_and_idempotence() {
    let service = Service::new();
    let data = service.root.path().join("data ' \" $ ` dir");
    let bin = data.join("jobmarshal/bin");
    for path in
        [None, Some(OsString::new()), Some(OsString::from_vec(b":/bin:quote'\"$`\xff:".to_vec()))]
    {
        let mut start = service.command();
        start.env("XDG_DATA_HOME", &data).args(["start", "-j", "1"]);
        if let Some(path) = &path {
            start.env("PATH", path);
        } else {
            start.env_remove("PATH");
        }
        let output = start.assert().success().get_output().stdout.clone();
        let mut script = output;
        script.extend_from_slice(b"printf '%s' \"$PATH\"");
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(OsString::from_vec(script))
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut expected = bin.as_os_str().as_bytes().to_vec();
        if let Some(path) = &path {
            expected.push(b':');
            expected.extend_from_slice(path.as_bytes());
        }
        assert_eq!(expected, output.stdout);
        service
            .command()
            .env("XDG_DATA_HOME", &data)
            .env("PATH", OsString::from_vec(expected))
            .args(["start", "-j", "1"])
            .assert()
            .success()
            .stdout(format!("export JOBMARSHAL_RUNTIME_DIR='{}'\n", service.runtime.display()));
    }
    let path = env::join_paths([PathBuf::from("/bin"), bin, PathBuf::from("/usr/bin")]).unwrap();
    service
        .command()
        .env("XDG_DATA_HOME", data)
        .env("PATH", path)
        .args(["start", "-j", "1"])
        .assert()
        .success()
        .stdout(format!("export JOBMARSHAL_RUNTIME_DIR='{}'\n", service.runtime.display()));
}

#[test]
fn test_default_and_relative_xdg_data_home() {
    let service = Service::new();
    install(&service);
    for value in [None, Some("relative"), Some("")] {
        let mut command = command(&service);
        command.env("HOME", service.root.path());
        if let Some(value) = value {
            command.env("XDG_DATA_HOME", value);
        } else {
            command.env_remove("XDG_DATA_HOME");
        }
        command.args(["add-wrapper", "cc"]).assert().success();
    }
    assert!(service.root.path().join(".local/share/jobmarshal/bin/cc").is_file());
    assert!(!data(&service).exists());
    assert!(!service.runtime.exists());
}
