// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Lifecycle, discovery, and malformed peer behavior.

use crate::common::service::{ChildGuard, Service};
use assert_cmd::Command as AssertCommand;
use jobmarshal_common::protocol::{Request, Response, receive, send};
use std::fs::{self, Permissions};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_transitions_and_stable_locks() {
    let service = Service::new();
    service.command().arg("status").assert().code(1).stdout("not running\n").stderr("");
    service.command().arg("stop").assert().code(1);
    assert_eq!(
        format!("export JOBMARSHAL_RUNTIME_DIR='{}'\n", service.runtime.display()),
        service.start("2")
    );
    let status = service.status();
    assert!(status.contains("jobs=2 active_builds=0 waiting_builds=0 draining=false"));
    service.start("2");
    assert_eq!(status, service.status());
    service.command().arg("stop").assert().success().stdout("").stderr("");
    assert!(!service.runtime.join("control.sock").exists());
    for name in ["daemon.lock", "lifecycle.lock", "daemon.log"] {
        assert!(service.runtime.join(name).is_file());
    }
    service.command().arg("status").assert().code(1);
    service.start("3");
    assert!(service.status().contains("jobs=3"));
}

#[test]
fn test_capacity_precedence_and_conflicts() {
    let service = Service::new();
    service.command().env("JOBMARSHAL_JOBS", "3").arg("start").assert().success();
    assert!(service.status().contains("jobs=3"));
    service.command().arg("start").assert().success();
    service.command().env("JOBMARSHAL_JOBS", "4").arg("start").assert().code(1);
    service.command().args(["start", "--jobs", "4"]).assert().code(1);
    service
        .command()
        .env("JOBMARSHAL_JOBS", "invalid")
        .args(["start", "-j", "3"])
        .assert()
        .success();
    for value in ["0", "-1", "bad", "184467440737095516160"] {
        service.command().args(["start", &format!("--jobs={value}")]).assert().code(1);
    }
}

#[test]
fn test_default_capacity() {
    let service = Service::new();
    let jobs = thread::available_parallelism().unwrap().get();
    service.command().arg("start").assert().success();
    assert!(service.status().contains(&format!("jobs={jobs} ")));
}

#[test]
fn test_concurrent_starters() {
    let service = Service::new();
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let runtime = service.runtime.clone();
            thread::spawn(move || {
                AssertCommand::new(env!("CARGO_BIN_EXE_jobmarshal"))
                    .timeout(Duration::from_secs(25))
                    .env("JOBMARSHAL_RUNTIME_DIR", runtime)
                    .env_remove("JOBMARSHAL_JOBS")
                    .args(["start", "-j", "2"])
                    .output()
                    .unwrap()
            })
        })
        .collect();
    for worker in workers {
        let output = worker.join().unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    assert!(service.status().contains("jobs=2"));
}

#[test]
fn test_stale_socket() {
    let service = Service::new();
    fs::create_dir(&service.runtime).unwrap();
    fs::set_permissions(&service.runtime, Permissions::from_mode(0o700)).unwrap();
    drop(UnixListener::bind(service.runtime.join("control.sock")).unwrap());
    service.command().arg("status").assert().code(1).stdout("not running\n");
    service.start("1");
    assert!(service.status().contains("jobs=1"));
}

#[test]
fn test_malformed_peers_and_concurrent_control() {
    let service = Service::new();
    service.start("2");
    for frame in
        [b"bad\n".to_vec(), b"{\"version\":99,\"body\":\"Status\"}\n".to_vec(), vec![b'x'; 8193]]
    {
        let mut stream = UnixStream::connect(service.runtime.join("control.sock")).unwrap();
        stream.write_all(&frame).unwrap();
        assert!(matches!(receive::<Response>(&mut stream).unwrap(), Response::Error(_)));
    }
    let _stalled = UnixStream::connect(service.runtime.join("control.sock")).unwrap();
    let start = Instant::now();
    assert!(service.status().contains("jobs=2"));
    assert!(start.elapsed() < Duration::from_secs(2));
    service.command().arg("stop").assert().success();
    assert!(!service.runtime.join("control.sock").exists());
}

#[test]
fn test_unsafe_runtime_and_files() {
    let service = Service::new();
    fs::create_dir(&service.runtime).unwrap();
    fs::set_permissions(&service.runtime, Permissions::from_mode(0o755)).unwrap();
    service.command().arg("start").assert().code(1);
    fs::set_permissions(&service.runtime, Permissions::from_mode(0o700)).unwrap();
    let target = service.root.path().join("target");
    fs::write(&target, "keep").unwrap();
    for name in ["lifecycle.lock", "daemon.lock", "daemon.log"] {
        symlink(&target, service.runtime.join(name)).unwrap();
        service.command().args(["start", "-j", "1"]).assert().code(1);
        fs::remove_file(service.runtime.join(name)).unwrap();
        assert_eq!("keep", fs::read_to_string(&target).unwrap());
    }
    fs::remove_file(service.runtime.join("control.sock")).unwrap();
    fs::write(service.runtime.join("control.sock"), "keep").unwrap();
    service.command().args(["start", "-j", "1"]).assert().code(1);
    assert_eq!("keep", fs::read_to_string(service.runtime.join("control.sock")).unwrap());
    fs::remove_file(service.runtime.join("control.sock")).unwrap();
    service.command().env("JOBMARSHAL_RUNTIME_DIR", "relative").arg("status").assert().code(1);
    service
        .command()
        .env("JOBMARSHAL_RUNTIME_DIR", service.root.path().join("has space"))
        .arg("status")
        .assert()
        .code(1);
}

#[test]
fn test_xdg_discovery() {
    let service = Service::new();
    let xdg = service.root.path().join("xdg");
    fs::create_dir(&xdg).unwrap();
    fs::set_permissions(&xdg, Permissions::from_mode(0o700)).unwrap();
    let runtime = xdg.join("jobmarshal");
    // Cleanup remains armed for the discovered service rather than the override fixture.
    let mut service = service;
    service.runtime = runtime;
    service
        .command()
        .env_remove("JOBMARSHAL_RUNTIME_DIR")
        .env("XDG_RUNTIME_DIR", xdg)
        .args(["start", "-j", "1"])
        .assert()
        .success();
    assert!(service.status().contains("jobs=1"));
}

#[test]
fn test_shell_quoted_export() {
    let mut service = Service::new();
    service.runtime = service.root.path().join("quote'\"$`directory");
    let export = service.start("1");
    let output = Command::new("sh")
        .args(["-c", &format!("{export}\nprintf '%s' \"$JOBMARSHAL_RUNTIME_DIR\"")])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(service.runtime.to_str().unwrap().as_bytes(), output.stdout);
}

#[test]
fn test_start_detaches_streams_and_logs_privately() {
    let service = Service::new();
    let output =
        service.command().args(["start", "-j", "1"]).assert().success().get_output().clone();
    assert!(output.stderr.is_empty());
    assert_eq!(
        0,
        fs::metadata(service.runtime.join("daemon.log")).unwrap().permissions().mode() & 0o077
    );
}

#[test]
fn test_start_rejects_draining_peer() {
    let service = Service::new();
    fs::create_dir(&service.runtime).unwrap();
    fs::set_permissions(&service.runtime, Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(service.runtime.join("control.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let peer = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        assert!(matches!(receive::<Request>(&mut stream).unwrap(), Request::Status));
        send(
            &mut stream,
            Response::Status { active: 0, draining: true, jobs: 1, pid: 1, waiting: 0 },
        )
        .unwrap();
    });
    service.command().args(["start", "-j", "1"]).assert().code(1);
    peer.join().unwrap();
    // Avoid cleanup trying to contact the fixture socket after its listener closes.
    fs::remove_file(service.runtime.join("control.sock")).unwrap();
}

#[test]
fn test_stalled_service_failure_is_bounded() {
    let service = Service::new();
    fs::create_dir(&service.runtime).unwrap();
    fs::set_permissions(&service.runtime, Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(service.runtime.join("control.sock")).unwrap();
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_jobmarshal"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .args(["start", "-j", "1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let _stream = loop {
        if let Ok((stream, _)) = listener.accept() {
            break stream;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    };
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        if Instant::now() > deadline {
            panic!("start failed to time out");
        }
        thread::sleep(Duration::from_millis(5));
    }
    drop(listener);
    fs::remove_file(service.runtime.join("control.sock")).unwrap();
}
