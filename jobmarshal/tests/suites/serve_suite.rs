// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Foreground supervision and graceful stop.

use crate::common::service::{ChildGuard, Service, wait};
use jobmarshal_common::{
    client,
    protocol::{Request, Response, receive, send},
    runtime::Paths,
};
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

#[test]
fn test_serve_owns_pid_streams_and_drains() {
    let service = Service::new();
    let stdout = fs::File::create(service.root.path().join("stdout")).unwrap();
    let stderr = fs::File::create(service.root.path().join("stderr")).unwrap();
    let mut child = ChildGuard(
        service
            .process()
            .args(["serve", "-j", "2"])
            .stdout(stdout)
            .stderr(stderr)
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait(|| service.runtime.join("control.sock").exists());
    let paths = Paths::prepare(service.runtime.clone()).unwrap();
    assert!(
        matches!(client::status(&paths).unwrap(), Some(Response::Status {pid, jobs: 2, ..}) if pid == child.0.id())
    );
    assert!(!service.runtime.join("daemon.log").exists());
    service.start("2");
    service.command().args(["serve", "-j", "2"]).assert().code(1).stdout("");
    let mut malformed = UnixStream::connect(paths.socket()).unwrap();
    malformed.write_all(b"invalid\n").unwrap();
    assert!(matches!(receive::<Response>(&mut malformed).unwrap(), Response::Error(_)));
    let mut lease = UnixStream::connect(paths.socket()).unwrap();
    send(&mut lease, Request::Acquire).unwrap();
    assert!(matches!(receive::<Response>(&mut lease).unwrap(), Response::Granted { .. }));
    let mut stop = ChildGuard(service.process().arg("stop").spawn().unwrap());
    wait(|| {
        matches!(client::status(&paths).unwrap(), Some(Response::Status { draining: true, .. }))
    });
    assert!(stop.0.try_wait().unwrap().is_none());
    assert!(child.0.try_wait().unwrap().is_none());
    drop(lease);
    wait(|| stop.0.try_wait().unwrap().is_some());
    wait(|| child.0.try_wait().unwrap().is_some());
    assert!(stop.0.wait().unwrap().success());
    assert!(child.0.wait().unwrap().success());
    assert!(!paths.socket().exists());
    assert!(!service.runtime.join("tokens").exists());
    assert_eq!("", fs::read_to_string(service.root.path().join("stdout")).unwrap());
    assert_eq!("", fs::read_to_string(service.root.path().join("stderr")).unwrap());
}

#[test]
fn test_serve_capacity_and_startup_diagnostics() {
    let service = Service::new();
    service.command().args(["serve", "--jobs=0"]).assert().code(1).stdout("");
    service.command().env("JOBMARSHAL_JOBS", "invalid").arg("serve").assert().code(1);
    service.start("2");
    service.command().arg("serve").assert().code(1).stdout("");
    service.command().arg("stop").assert().success();
    fs::write(service.runtime.join("tokens"), "keep").unwrap();
    service.command().arg("serve").assert().code(1).stdout("");
    assert_eq!("keep", fs::read_to_string(service.runtime.join("tokens")).unwrap());
}
