// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Counter, reset, and observational native-token behavior.

use crate::common::{
    BoundedCommand, ChildGuard, Service, binary, fixture, held, ready, release, wait, wrapper,
};
use jobmarshal_common::{
    client,
    protocol::{Request, Response, receive, send},
    runtime::Paths,
    stats::Stats,
};
use nix::fcntl::{FcntlArg, fcntl};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::process::Command;

fn stats(service: &Service, zero: bool) -> Stats {
    client::stats(&Paths::prepare(service.runtime.clone()).unwrap(), zero).unwrap()
}

#[test]
fn test_stats_absent_does_not_start_service() {
    let service = Service::new();
    for arguments in [vec!["stats"], vec!["stats", "--zero"]] {
        service.command().args(arguments).assert().code(1).stdout("");
        assert!(!service.runtime.join("control.sock").exists());
        assert!(!service.runtime.join("daemon.lock").exists());
    }
}

#[test]
fn test_stats_idle_labels_and_restart() {
    let service = Service::new();
    service.start("2");
    let initial = stats(&service, false);
    assert_eq!(2, initial.jobs);
    assert_eq!(2, initial.available);
    assert_eq!(0, initial.unavailable);
    assert_eq!(0, initial.active_builds);
    assert_eq!(0, initial.processed_builds);
    assert!(!initial.draining);
    assert_eq!(initial.started, initial.reset);
    assert_eq!(service.runtime.join("control.sock"), initial.socket);
    assert_eq!(service.runtime.join("tokens"), initial.fifo);
    let output =
        service.command().arg("stats").assert().success().stderr("").get_output().stdout.clone();
    let text = String::from_utf8(output).unwrap();
    let labels = [
        "Listening socket",
        "Token FIFO",
        "Server PID",
        "Server started",
        "Stats last reset",
        "Max jobs",
        "Tokens available",
        "Tokens unavailable",
        "Active builds",
        "Builds processed",
        "Draining",
    ];
    for (line, label) in text.lines().zip(labels) {
        assert!(line.starts_with(&format!("{label:<22}")));
    }
    for label in ["Server started", "Stats last reset"] {
        let line = text.lines().find(|line| line.starts_with(label)).unwrap();
        let value = &line[22..];
        assert!(value.contains('T') && value.ends_with('Z'));
    }
    assert!(text.ends_with(
        "Token counts are a snapshot; job demand and job wait times are not measured.\n"
    ));
    assert!(wrapper(&service).arg(fixture()).args(["exit", "0"]).bounded_status().success());
    wait(|| stats(&service, false).processed_builds == 1);
    service.command().arg("stop").assert().success();
    service.start("2");
    let restarted = stats(&service, false);
    assert_ne!(initial.pid, restarted.pid);
    assert!(restarted.started >= initial.started);
    assert_eq!(restarted.started, restarted.reset);
    assert_eq!(0, restarted.processed_builds);
    assert_eq!(2, restarted.available);
}

#[test]
fn test_stats_reset_counts_active_completion_and_failed_commands() {
    let service = Service::new();
    let mut active = held(&service, "active");
    ready(&service, "active");
    let initial = stats(&service, false);
    assert_eq!(1, initial.active_builds);
    assert_eq!(1, initial.available);
    assert_eq!(1, initial.unavailable);
    assert_eq!(
        Some(17),
        wrapper(&service).arg(fixture()).args(["exit", "17"]).bounded_status().code()
    );
    wait(|| stats(&service, false).processed_builds == 1);
    let reset = stats(&service, true);
    assert_eq!(initial.started, reset.started);
    assert!(reset.reset > initial.reset);
    assert_eq!(0, reset.processed_builds);
    assert_eq!(1, reset.active_builds);
    assert_eq!(1, reset.available);
    release(&service, "active");
    wait(|| active.0.try_wait().unwrap().is_some());
    wait(|| stats(&service, false).processed_builds == 1);
    let completed = stats(&service, false);
    assert_eq!(reset.reset, completed.reset);
    assert_eq!(2, completed.available);
    assert_eq!(0, completed.active_builds);
    assert_eq!(
        Some(125),
        wrapper(&service).arg("/missing-jobmarshal-test-command").bounded_status().code()
    );
    wait(|| stats(&service, false).processed_builds == 2);
    let output =
        service.command().args(["stats", "--zero"]).assert().success().get_output().stdout.clone();
    assert!(String::from_utf8(output).unwrap().contains("Builds processed      0\n"));
    assert_eq!(0, stats(&service, false).processed_builds);
}

#[test]
fn test_stats_queued_and_draining_rejections_do_not_count() {
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
    wait(|| service.status().contains("waiting_builds=1"));
    for _ in 0..10 {
        let snapshot = stats(&service, false);
        assert_eq!(1, snapshot.active_builds);
        assert_eq!(0, snapshot.available);
        assert_eq!(0, snapshot.processed_builds);
    }
    let mut stop = ChildGuard(
        Command::new(binary("jobmarshal"))
            .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
            .arg("stop")
            .spawn()
            .unwrap(),
    );
    wait(|| stats(&service, false).draining);
    wait(|| queued.0.try_wait().unwrap().is_some());
    assert_eq!(Some(125), queued.0.wait().unwrap().code());
    let paths = Paths::prepare(service.runtime.clone()).unwrap();
    let mut rejected = client::connect(&paths).unwrap().unwrap();
    send(&mut rejected, Request::Acquire).unwrap();
    assert!(matches!(receive::<Response>(&mut rejected).unwrap(), Response::Error(_)));
    let snapshot = stats(&service, true);
    assert!(snapshot.draining);
    assert_eq!(1, snapshot.active_builds);
    assert_eq!(0, snapshot.available);
    assert_eq!(0, snapshot.processed_builds);
    release(&service, "active");
    wait(|| active.0.try_wait().unwrap().is_some());
    wait(|| stop.0.try_wait().unwrap().is_some());
    assert!(stop.0.wait().unwrap().success());
}

#[test]
fn test_stats_native_tokens_are_observed_without_consumption_or_flag_changes() {
    let service = Service::new();
    service.start("3");
    let mut native =
        OpenOptions::new().read(true).write(true).open(service.runtime.join("tokens")).unwrap();
    let flags = fcntl(native.as_fd(), FcntlArg::F_GETFL).unwrap();
    let mut token = [0];
    native.read_exact(&mut token).unwrap();
    for _ in 0..10 {
        let snapshot = stats(&service, false);
        assert_eq!(2, snapshot.available);
        assert_eq!(1, snapshot.unavailable);
        assert_eq!(0, snapshot.active_builds);
        assert_eq!(flags, fcntl(native.as_fd(), FcntlArg::F_GETFL).unwrap());
    }
    native.write_all(&token).unwrap();
    assert_eq!(3, stats(&service, false).available);
    native.read_exact(&mut token).unwrap();
    drop(native);
    for zero in [false, true, false] {
        let snapshot = stats(&service, zero);
        assert_eq!(2, snapshot.available);
        assert_eq!(1, snapshot.unavailable);
        assert_eq!(0, snapshot.processed_builds);
    }
    let mut first = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("first"))
            .spawn()
            .unwrap(),
    );
    let mut second = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("second"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "first");
    ready(&service, "second");
    let mut queued = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .args(["exit", "0"])
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("active_builds=2 waiting_builds=1"));
    for _ in 0..10 {
        assert_eq!(0, stats(&service, false).available);
    }
    assert!(queued.0.try_wait().unwrap().is_none());
    release(&service, "first");
    release(&service, "second");
    wait(|| first.0.try_wait().unwrap().is_some());
    wait(|| second.0.try_wait().unwrap().is_some());
    wait(|| queued.0.try_wait().unwrap().is_some());
    wait(|| stats(&service, false).processed_builds == 3);
    assert_eq!(2, stats(&service, false).available);
}

#[test]
fn test_stats_reports_invalid_observations_and_protocol_responses() {
    let service = Service::new();
    service.start("1");
    let mut native =
        OpenOptions::new().read(true).write(true).open(service.runtime.join("tokens")).unwrap();
    native.write_all(b"+").unwrap();
    let output =
        service.command().arg("stats").assert().code(1).stdout("").get_output().stderr.clone();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("cannot collect statistics: token availability exceeds configured capacity")
    );
    let mut byte = [0];
    native.read_exact(&mut byte).unwrap();
    assert_eq!(1, stats(&service, false).available);
    service.command().arg("stop").assert().success();
    let listener =
        std::os::unix::net::UnixListener::bind(service.runtime.join("control.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let peer = std::thread::spawn(move || {
        let mut stream = None;
        wait(|| match listener.accept() {
            Ok((accepted, _)) => {
                stream = Some(accepted);
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("{error}"),
        });
        let mut stream = stream.unwrap();
        assert!(matches!(receive::<Request>(&mut stream).unwrap(), Request::Stats { zero: false }));
        send(&mut stream, Response::Stopped).unwrap();
    });
    let output =
        service.command().arg("stats").assert().code(1).stdout("").get_output().stderr.clone();
    assert!(String::from_utf8(output).unwrap().contains("unexpected stats response"));
    peer.join().unwrap();
}
