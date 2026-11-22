// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Transparent process execution and PATH alias resolution.

use crate::common::{
    BoundedCommand, ChildGuard, Service, binary, copy_executable, fixture, wait, wrapper,
    wrapper_at,
};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::symlink;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};

#[test]
fn test_arguments_environment_cwd_and_streams() {
    let service = Service::new();
    let mut child = ChildGuard(
        wrapper(&service)
            .arg(fixture())
            .arg("forward")
            .arg("--child-option")
            .arg(OsString::from_vec(b"raw\xff".to_vec()))
            .env("FIXTURE_VALUE", "kept")
            .env("MAKEFLAGS", "--keep-going")
            .current_dir(service.root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    child.0.stdin.take().unwrap().write_all(b"stdin preserved").unwrap();
    wait(|| child.0.try_wait().unwrap().is_some());
    let mut stdout = Vec::new();
    std::io::Read::read_to_end(&mut child.0.stdout.take().unwrap(), &mut stdout).unwrap();
    assert!(stdout.starts_with(b"--child-option\0raw\xff\0"));
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains(&format!("cwd={}\n", service.root.path().display())));
    assert!(text.contains("env=kept\n"));
    assert!(text.contains("MAKEFLAGS=--keep-going -j --jobserver-auth=fifo:"));
    assert!(text.ends_with("stdin preserved"));
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut child.0.stderr.take().unwrap(), &mut stderr).unwrap();
    assert_eq!("fixture stderr\n", stderr);
}

#[test]
fn test_exit_and_signals() {
    let service = Service::new();
    assert_eq!(
        Some(37),
        wrapper(&service).arg(fixture()).args(["exit", "37"]).bounded_status().code()
    );
    let status = wrapper(&service).arg("sh").args(["-c", "kill -TERM $$"]).bounded_status();
    assert_eq!(Some(15), status.signal());
    assert_eq!(Some(125), wrapper(&service).bounded_status().code());
}

#[test]
fn test_nested_capacity_one() {
    let service = Service::new();
    service.start("1");
    let status = wrapper(&service)
        .env("JOBMARSHAL_JOBS", "1")
        .arg(fixture())
        .arg("nested")
        .arg(binary("jobmarshal-wrapper"))
        .arg(fixture())
        .args(["exit", "0"])
        .bounded_status();
    assert!(status.success());
}

#[test]
fn test_hardlinks_skip_aliases_and_preserve_arg0() {
    let service = Service::new();
    for directory in ["anchor", "alias", "real"] {
        fs::create_dir(service.root.path().join(directory)).unwrap();
    }
    let anchor = service.root.path().join("anchor");
    copy_executable(binary("jobmarshal-wrapper"), service.root.path().join("wrapper"));
    fs::hard_link(service.root.path().join("wrapper"), anchor.join("build-tool")).unwrap();
    copy_executable(binary("jobmarshal"), anchor.join("jobmarshal"));
    symlink(anchor.join("build-tool"), service.root.path().join("alias/build-tool")).unwrap();
    let real = service.root.path().join("real/build-tool");
    copy_executable(fixture(), &real);
    let path = std::env::join_paths([
        &anchor,
        &service.root.path().join("alias"),
        &service.root.path().join("real"),
    ])
    .unwrap();
    let output = wrapper_at(&service, &anchor.join("build-tool"))
        .env("PATH", path)
        .args(["arg0", "argument"])
        .bounded_output();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        format!("{}\nargument\n", anchor.join("build-tool").display()).as_bytes(),
        output.stdout
    );
}

#[test]
fn test_path_empty_entries_and_missing_anchor_or_real_command() {
    let service = Service::new();
    let anchor = service.root.path().join("anchor");
    fs::create_dir(&anchor).unwrap();
    copy_executable(binary("jobmarshal-wrapper"), service.root.path().join("wrapper"));
    fs::hard_link(service.root.path().join("wrapper"), anchor.join("tool")).unwrap();
    copy_executable(binary("jobmarshal"), anchor.join("jobmarshal"));
    let real = service.root.path().join("tool");
    copy_executable(fixture(), &real);
    let path = format!("{}:", anchor.display());
    let status = wrapper_at(&service, &anchor.join("tool"))
        .arg0("tool")
        .env("PATH", path)
        .args(["exit", "23"])
        .current_dir(service.root.path())
        .bounded_status();
    assert_eq!(Some(23), status.code());
    assert_eq!(
        Some(125),
        wrapper_at(&service, &anchor.join("tool"))
            .arg0("tool")
            .env("PATH", service.root.path())
            .bounded_status()
            .code()
    );
    assert_eq!(
        Some(125),
        wrapper_at(&service, &anchor.join("tool"))
            .arg0("tool")
            .env("PATH", &anchor)
            .bounded_status()
            .code()
    );
}

#[test]
fn test_rustup_proxy_dispatch() {
    let service = Service::new();
    let anchor = service.root.path().join("anchor");
    fs::create_dir(&anchor).unwrap();
    copy_executable(binary("jobmarshal-wrapper"), service.root.path().join("wrapper"));
    fs::hard_link(service.root.path().join("wrapper"), anchor.join("rustc")).unwrap();
    copy_executable(binary("jobmarshal"), anchor.join("jobmarshal"));
    let mut paths = vec![anchor.clone()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let output = Command::new(anchor.join("rustc"))
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
        .env("JOBMARSHAL_JOBS", "2")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .arg("--version")
        .bounded_output();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8(output.stdout).unwrap().starts_with("rustc "));
}
