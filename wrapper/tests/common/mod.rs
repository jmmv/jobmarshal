// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Native protocol and subprocess fixtures.

#[path = "../../../tests/common/mod.rs"]
mod shared;
pub(crate) use shared::{ChildGuard, Service, binary, copy_executable, wait};

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::OnceLock;
use tempfile::{TempDir, tempdir};

pub(crate) trait BoundedCommand {
    fn bounded_output(&mut self) -> Output;
    fn bounded_status(&mut self) -> ExitStatus {
        self.bounded_output().status
    }
}

impl BoundedCommand for Command {
    fn bounded_output(&mut self) -> Output {
        let mut child =
            ChildGuard(self.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap());
        wait(|| child.0.try_wait().unwrap().is_some());
        let status = child.0.wait().unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        child.0.stdout.take().unwrap().read_to_end(&mut stdout).unwrap();
        child.0.stderr.take().unwrap().read_to_end(&mut stderr).unwrap();
        Output { status, stdout, stderr }
    }
}

pub(crate) fn wrapper(service: &Service) -> Command {
    wrapper_at(service, &binary("jobmarshal-wrapper"))
}

pub(crate) fn wrapper_at(service: &Service, path: &Path) -> Command {
    let mut command = Command::new(path);
    command
        .env("JOBMARSHAL_RUNTIME_DIR", &service.runtime)
        .env("JOBMARSHAL_JOBS", "2")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS");
    command
}

pub(crate) fn fixture() -> PathBuf {
    static FIXTURE: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let root = tempdir().unwrap();
            let executable = root.path().join("fixture");
            let status = Command::new("rustc")
                .args(["--edition=2024", "-o"])
                .arg(&executable)
                .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native.rs"))
                .status()
                .unwrap();
            assert!(status.success());
            (root, executable)
        })
        .1
        .clone()
}

pub(crate) fn held(service: &Service, name: &str) -> ChildGuard {
    ChildGuard(
        wrapper(service)
            .arg(fixture())
            .arg("hold")
            .arg(service.root.path().join(name))
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

pub(crate) fn ready(service: &Service, name: &str) {
    wait(|| service.root.path().join(format!("{name}.ready")).exists());
}

pub(crate) fn release(service: &Service, name: &str) {
    fs::write(service.root.path().join(format!("{name}.release")), "").unwrap();
}
