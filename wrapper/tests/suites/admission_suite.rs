// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Admission, draining, and lifetime ownership.

use crate::common::{
    BoundedCommand, ChildGuard, Service, binary, fixture, held, ready, release, wait, wrapper,
};
use jobmarshal_common::{
    client,
    protocol::{Request, Response, receive, send},
    runtime::Paths,
};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn counts(service: &Service, active: usize, waiting: usize) {
    wait(|| service.status().contains(&format!("active_builds={active} waiting_builds={waiting}")));
}

#[test]
fn test_admission_ceiling_and_queued_disconnect() {
    let service = Service::new();
    let mut first = held(&service, "first");
    let mut second = held(&service, "second");
    ready(&service, "first");
    ready(&service, "second");
    counts(&service, 2, 0);
    let mut queued = held(&service, "queued");
    counts(&service, 2, 1);
    assert!(!service.root.path().join("queued.ready").exists());
    queued.0.kill().unwrap();
    queued.0.wait().unwrap();
    counts(&service, 2, 0);
    let mut third = held(&service, "third");
    counts(&service, 2, 1);
    release(&service, "first");
    wait(|| first.0.try_wait().unwrap().is_some());
    ready(&service, "third");
    counts(&service, 2, 0);
    release(&service, "second");
    release(&service, "third");
    wait(|| second.0.try_wait().unwrap().is_some());
    wait(|| third.0.try_wait().unwrap().is_some());
    counts(&service, 0, 0);
}

#[test]
fn test_failed_exec_and_abandoned_grant_release_token() {
    let service = Service::new();
    service.start("2");
    assert_eq!(Some(125), wrapper(&service).arg("/no/such/command").bounded_status().code());
    counts(&service, 0, 0);
    let paths = Paths::prepare(service.runtime.clone()).unwrap();
    let mut stream = client::connect(&paths).unwrap().unwrap();
    send(&mut stream, Request::Acquire).unwrap();
    drop(stream);
    counts(&service, 0, 0);
    let mut stream = client::connect(&paths).unwrap().unwrap();
    send(&mut stream, Request::Acquire).unwrap();
    assert!(matches!(receive::<Response>(&mut stream).unwrap(), Response::Granted { .. }));
    counts(&service, 1, 0);
    drop(stream);
    counts(&service, 0, 0);
}

#[test]
fn test_stop_drains_and_rejects_queued_and_new() {
    let service = Service::new();
    service.start("1");
    let mut active = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("active"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "active");
    let mut queued = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("queued"))
            .spawn()
            .unwrap(),
    );
    counts(&service, 1, 1);
    let mut stop = ChildGuard(
        Command::new(binary("jobmarshal"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .arg("stop")
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("draining=true"));
    wait(|| queued.0.try_wait().unwrap().is_some());
    assert_eq!(Some(125), queued.0.wait().unwrap().code());
    let paths = Paths::prepare(service.runtime.clone()).unwrap();
    let mut new = client::connect(&paths).unwrap().unwrap();
    send(&mut new, Request::Acquire).unwrap();
    assert!(matches!(receive::<Response>(&mut new).unwrap(), Response::Error(_)));
    assert!(stop.0.try_wait().unwrap().is_none());
    release(&service, "active");
    wait(|| active.0.try_wait().unwrap().is_some());
    wait(|| stop.0.try_wait().unwrap().is_some());
    assert!(stop.0.wait().unwrap().success());
    assert!(!service.runtime.join("control.sock").exists());
    service.start("2");
}

#[test]
fn test_crash_requires_surviving_participants_to_exit() {
    let service = Service::new();
    let mut active = held(&service, "active");
    ready(&service, "active");
    let status = service.status();
    let pid: i32 = status.split("pid=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    kill(Pid::from_raw(pid), Signal::SIGKILL).unwrap();
    wait(|| !service.command().arg("status").output().unwrap().status.success());
    service.command().args(["start", "-j", "2"]).assert().code(1);
    release(&service, "active");
    wait(|| active.0.try_wait().unwrap().is_some());
    service.start("2");
    let mut recovered = held(&service, "recovered");
    ready(&service, "recovered");
    release(&service, "recovered");
    wait(|| recovered.0.try_wait().unwrap().is_some());
}

#[test]
fn test_fifo_capacity_is_bounded_and_unrelated_path_preserved() {
    let service = Service::new();
    service.command().args(["start", "-j", "1000000000"]).assert().code(1);
    fs::write(service.runtime.join("tokens"), "keep").unwrap();
    service.command().args(["start", "-j", "1"]).assert().code(1);
    assert_eq!("keep", fs::read_to_string(service.runtime.join("tokens")).unwrap());
}

#[test]
fn test_admission_and_stop_wait_beyond_control_deadline() {
    let service = Service::new();
    service.start("1");
    let mut active = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("active"))
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    ready(&service, "active");
    counts(&service, 1, 0);
    let mut queued = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("exit")
            .arg("0")
            .spawn()
            .unwrap(),
    );
    counts(&service, 1, 1);
    let start = Instant::now();
    wait(|| {
        assert!(queued.0.try_wait().unwrap().is_none());
        start.elapsed() > jobmarshal_common::protocol::TIMEOUT + Duration::from_millis(100)
    });
    release(&service, "active");
    wait(|| queued.0.try_wait().unwrap().is_some());
    assert!(queued.0.wait().unwrap().success());
    wait(|| active.0.try_wait().unwrap().is_some());
    let mut active = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("second"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "second");
    counts(&service, 1, 0);
    let mut stop = ChildGuard(
        Command::new(binary("jobmarshal"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .arg("stop")
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("draining=true"));
    let start = Instant::now();
    wait(|| {
        assert!(stop.0.try_wait().unwrap().is_none());
        start.elapsed() > jobmarshal_common::protocol::TIMEOUT + Duration::from_millis(100)
    });
    release(&service, "second");
    wait(|| stop.0.try_wait().unwrap().is_some());
    assert!(stop.0.wait().unwrap().success());
    wait(|| active.0.try_wait().unwrap().is_some());
}

#[test]
fn test_nested_work_finishes_while_draining() {
    let service = Service::new();
    service.start("1");
    let mut active = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "1")
            .arg(fixture())
            .arg("nested-gated")
            .arg(service.root.path().join("nested"))
            .arg(binary("jobmarshal-wrapper"))
            .arg(fixture())
            .args(["exit", "0"])
            .spawn()
            .unwrap(),
    );
    ready(&service, "nested");
    let mut stop = ChildGuard(
        Command::new(binary("jobmarshal"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .arg("stop")
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("draining=true"));
    release(&service, "nested");
    wait(|| active.0.try_wait().unwrap().is_some());
    assert!(active.0.wait().unwrap().success());
    wait(|| stop.0.try_wait().unwrap().is_some());
    assert!(stop.0.wait().unwrap().success());
}

#[test]
fn test_last_descendant_retains_lease() {
    struct Descendant(Option<Pid>);
    impl Drop for Descendant {
        fn drop(&mut self) {
            if let Some(pid) = self.0 {
                let _ = kill(pid, Signal::SIGKILL);
            }
        }
    }
    let service = Service::new();
    let mut parent = ChildGuard(
        wrapper(&service)
            .arg(fixture())
            .arg("descendant")
            .arg(service.root.path().join("descendant"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "descendant");
    let pid =
        fs::read_to_string(service.root.path().join("descendant.ready")).unwrap().parse().unwrap();
    let mut descendant = Descendant(Some(Pid::from_raw(pid)));
    wait(|| parent.0.try_wait().unwrap().is_some());
    counts(&service, 1, 0);
    release(&service, "descendant");
    counts(&service, 0, 0);
    descendant.0 = None;
}

#[test]
fn test_lease_returns_the_exact_reserved_byte() {
    use std::io::{Read, Write};
    use std::os::unix::fs::OpenOptionsExt;
    let service = Service::new();
    service.start("2");
    let mut fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK)
        .open(service.runtime.join("tokens"))
        .unwrap();
    let mut initial = [0; 2];
    fifo.read_exact(&mut initial).unwrap();
    fifo.write_all(b"xy").unwrap();
    let paths = Paths::prepare(service.runtime.clone()).unwrap();
    let mut stream = client::connect(&paths).unwrap().unwrap();
    send(&mut stream, Request::Acquire).unwrap();
    assert!(matches!(receive::<Response>(&mut stream).unwrap(), Response::Granted { .. }));
    let mut byte = [0];
    fifo.read_exact(&mut byte).unwrap();
    assert_eq!(b"y", &byte);
    drop(stream);
    counts(&service, 0, 0);
    fifo.read_exact(&mut byte).unwrap();
    assert_eq!(b"x", &byte);
    fifo.write_all(b"xy").unwrap();
}
