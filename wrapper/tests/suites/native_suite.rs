// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! External native allocation and conservative token loss.

use crate::common::{
    BoundedCommand, ChildGuard, Service, fixture, held, ready, release, wait, wrapper,
};
use jobserver::Client;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;

#[test]
fn test_external_inherited_pipe_without_daemon() {
    let service = Service::new();
    let client = Client::new(0).unwrap();
    let mut command = wrapper(&service);
    command.arg(fixture()).args(["exit", "19"]);
    client.configure(&mut command);
    assert_eq!(Some(19), command.bounded_status().code());
    assert!(!service.runtime.exists());
}

#[test]
fn test_invalid_auth_and_non_fifo_auth_fail_closed() {
    let service = Service::new();
    let regular = service.root.path().join("regular");
    fs::write(&regular, "tokens").unwrap();
    for flags in [
        "--jobserver-auth=999999,999998".to_string(),
        "--jobserver-auth=bad".into(),
        format!("--jobserver-auth=fifo:{}", regular.display()),
        format!("--jobserver-fds=fifo:{}", regular.display()),
    ] {
        assert_eq!(
            Some(125),
            wrapper(&service)
                .env("MAKEFLAGS", flags)
                .arg("sh")
                .args(["-c", "exit 0"])
                .bounded_status()
                .code()
        );
    }
    assert!(!service.runtime.exists());
}

#[test]
fn test_native_holders_and_token_loss_reduce_available_capacity() {
    let service = Service::new();
    service.start("3");
    let mut holder = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("lose")
            .arg(service.root.path().join("native"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "native");
    let mut other = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("other"))
            .spawn()
            .unwrap(),
    );
    ready(&service, "other");
    wait(|| service.status().contains("active_builds=2 waiting_builds=0"));
    let mut queued = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("queued"))
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("waiting_builds=1"));
    release(&service, "native");
    wait(|| holder.0.try_wait().unwrap().is_some());
    ready(&service, "queued");
    wait(|| service.status().contains("active_builds=2 waiting_builds=0"));
    let mut fourth = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join("fourth"))
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("waiting_builds=1"));
    assert!(!service.root.path().join("fourth.ready").exists());
    release(&service, "other");
    ready(&service, "fourth");
    release(&service, "queued");
    release(&service, "fourth");
    wait(|| other.0.try_wait().unwrap().is_some());
    wait(|| queued.0.try_wait().unwrap().is_some());
    wait(|| fourth.0.try_wait().unwrap().is_some());
    wait(|| service.status().contains("active_builds=0 waiting_builds=0"));
    // Two bytes remain: the native holder deliberately lost one, and snapshots/admissions must not
    // replenish it.
    let mut fifo = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK)
        .open(service.runtime.join("tokens"))
        .unwrap();
    let mut bytes = [0; 8];
    assert_eq!(2, fifo.read(&mut bytes).unwrap());
    fifo.write_all(&bytes[..2]).unwrap();
}

#[test]
fn test_native_read_flags_remain_blocking() {
    let service = Service::new();
    let mut active = held(&service, "active");
    ready(&service, "active");
    let file =
        OpenOptions::new().read(true).write(true).open(service.runtime.join("tokens")).unwrap();
    use std::os::fd::AsFd;
    let flags = nix::fcntl::fcntl(file.as_fd(), nix::fcntl::FcntlArg::F_GETFL).unwrap();
    assert_eq!(0, flags & nix::libc::O_NONBLOCK);
    release(&service, "active");
    wait(|| active.0.try_wait().unwrap().is_some());
}

#[test]
fn test_inherited_flags_are_preserved_and_selected_with_native_precedence() {
    let service = Service::new();
    let client = Client::new(0).unwrap();
    let mut command = wrapper(&service);
    command
        .arg(fixture())
        .arg("forward")
        .env("FIXTURE_VALUE", "external")
        .env("MAKEFLAGS", "--keep-going")
        .env("MFLAGS", "--silent");
    client.configure(&mut command);
    let output = command.bounded_output();
    assert!(output.status.success());
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("MAKEFLAGS=--keep-going -j --jobserver-fds="));
    assert!(output.contains("MFLAGS=--silent -j --jobserver-fds="));
    assert!(!service.runtime.exists());
}
