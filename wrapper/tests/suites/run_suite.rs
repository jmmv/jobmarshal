// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Explicit execution through installed and relocated layouts.

use crate::common::{
    BoundedCommand, ChildGuard, Service, binary, copy_executable, fixture, wait, wrapper,
};
use std::env;
use std::ffi::OsString;
use std::fs::{self, Permissions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn install(service: &Service) -> PathBuf {
    let root = service.root.path().join("installation with spaces");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("libexec")).unwrap();
    copy_executable(binary("jobmarshal"), root.join("bin/jobmarshal"));
    copy_executable(binary("jobmarshal-wrapper"), root.join("libexec/jobmarshal-wrapper"));
    root
}

fn run(service: &Service, root: &Path) -> Command {
    let mut paths = vec![root.join("bin")];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap()));
    let mut command = Command::new(root.join("bin/jobmarshal"));
    command
        .arg("run")
        .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
        .env("XDG_DATA_HOME", service.root.path().join("data"))
        .env("JOBMARSHAL_JOBS", "2")
        .env("PATH", env::join_paths(paths).unwrap())
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS");
    command
}

#[test]
fn test_run_path_explicit_optional_separator_and_exit() {
    let service = Service::new();
    let root = install(&service);
    let output = run(&service, &root).args(["sh", "-c", "exit 0"]).bounded_output();
    assert!(output.status.success(), "{output:?}");
    assert!(service.status().contains("jobs=2"));
    assert_eq!(
        Some(23),
        run(&service, &root).arg("--").arg(fixture()).args(["exit", "23"]).bounded_status().code()
    );
    assert_eq!(
        Some(125),
        run(&service, &root).arg("missing-jobmarshal-test-command").bounded_status().code()
    );
    assert_eq!(
        Some(15),
        run(&service, &root).args(["sh", "-c", "kill -TERM $$"]).bounded_status().signal()
    );
}

#[test]
fn test_run_preserves_os_strings_child_options_and_streams() {
    let service = Service::new();
    let root = install(&service);
    let raw_command = root.join(OsString::from_vec(b"command-\xff".to_vec()));
    copy_executable(fixture(), &raw_command);
    let mut child = ChildGuard(
        run(&service, &root)
            .arg(&raw_command)
            .arg("forward")
            .args(["--help", "--", "-j", "7", ""])
            .arg(OsString::from_vec(b"argument-\xff".to_vec()))
            .env("FIXTURE_VALUE", "retained")
            .current_dir(service.root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    child.0.stdin.take().unwrap().write_all(b"input preserved").unwrap();
    wait(|| child.0.try_wait().unwrap().is_some());
    assert!(child.0.wait().unwrap().success());
    let mut stdout = Vec::new();
    child.0.stdout.take().unwrap().read_to_end(&mut stdout).unwrap();
    assert!(stdout.starts_with(b"--help\0--\0-j\x007\0\0argument-\xff\0"));
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains(&format!("cwd={}\n", service.root.path().display())));
    assert!(text.contains("env=retained\n"));
    assert!(text.ends_with("input preserved"));
    let mut stderr = String::new();
    child.0.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    assert_eq!("fixture stderr\n", stderr);
}

#[test]
fn test_run_missing_command_and_broken_layout_errors() {
    let service = Service::new();
    let root = install(&service);
    for separator in [false, true] {
        let mut command = run(&service, &root);
        if separator {
            command.arg("--");
        }
        let output = command.bounded_output();
        assert_eq!(Some(2), output.status.code());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("jobmarshal help run"));
    }
    assert!(!service.runtime.exists());
    let wrapper = root.join("libexec/jobmarshal-wrapper");
    fs::set_permissions(&wrapper, Permissions::from_mode(0o600)).unwrap();
    for remove in [false, true] {
        if remove {
            fs::remove_file(&wrapper).unwrap();
        }
        let output = run(&service, &root).args(["sh", "-c", "exit 0"]).bounded_output();
        assert_eq!(Some(1), output.status.code());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("cannot execute wrapper"));
        assert!(stderr.contains("../libexec/jobmarshal-wrapper"));
        assert!(!service.runtime.exists());
    }
}

#[test]
fn test_run_relocates_with_the_installation() {
    let service = Service::new();
    let original = install(&service);
    assert!(run(&service, &original).arg(fixture()).args(["exit", "0"]).bounded_status().success());
    service.command().arg("stop").assert().success();
    let relocated = service.root.path().join("relocated installation");
    fs::rename(&original, &relocated).unwrap();
    assert!(!original.exists());
    assert_eq!(
        Some(29),
        run(&service, &relocated).arg(fixture()).args(["exit", "29"]).bounded_status().code()
    );
    assert!(service.status().contains("jobs=2"));
}

#[test]
fn test_run_and_wrapper_nesting_at_capacity_one() {
    let service = Service::new();
    let root = install(&service);
    service.start("1");
    let jobmarshal = root.join("bin/jobmarshal");
    let installed_wrapper = root.join("libexec/jobmarshal-wrapper");
    let output = run(&service, &root)
        .env("JOBMARSHAL_JOBS", "1")
        .arg(fixture())
        .arg("nested")
        .arg(&jobmarshal)
        .arg("run")
        .arg(fixture())
        .args(["exit", "0"])
        .bounded_output();
    assert!(output.status.success(), "{output:?}");
    assert!(
        run(&service, &root)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("nested")
            .arg(installed_wrapper)
            .arg(fixture())
            .args(["exit", "0"])
            .bounded_status()
            .success()
    );
    assert!(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(jobmarshal)
            .arg("run")
            .arg(fixture())
            .args(["exit", "0"])
            .bounded_status()
            .success()
    );
}
