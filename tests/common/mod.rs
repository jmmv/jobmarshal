// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Isolated daemon ownership for integration tests.

use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{TempDir, tempdir};

pub(crate) fn binary(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .ancestors()
        .skip(1)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("build workspace binaries before testing: missing {name}"))
}

pub(crate) fn copy_executable(source: impl AsRef<Path>, destination: impl AsRef<Path>) {
    // Keep writable executable descriptors out of this multithreaded process, where concurrent
    // forks could inherit them and cause ETXTBSY when another test executes the copy.
    let output =
        ProcessCommand::new("cp").arg(source.as_ref()).arg(destination.as_ref()).output().unwrap();
    assert!(output.status.success(), "{output:?}");
}

pub(crate) fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "condition did not become ready");
        thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) struct ChildGuard(pub(crate) Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(crate) struct Service {
    pub(crate) root: TempDir,
    pub(crate) runtime: PathBuf,
}

impl Service {
    pub(crate) fn new() -> Self {
        let root = tempdir().unwrap();
        let runtime = root.path().join("runtime");
        Self { root, runtime }
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(binary("jobmarshal"));
        command
            .env("JOBMARSHAL_RUNTIME_DIR", &self.runtime)
            .env_remove("JOBMARSHAL_JOBS")
            .env("HOME", self.root.path())
            .env("XDG_DATA_HOME", self.root.path().join("data"))
            .timeout(Duration::from_secs(25));
        command
    }

    pub(crate) fn start(&self, jobs: &str) -> String {
        let output = self
            .command()
            .args(["start", "-j", jobs])
            .assert()
            .success()
            .stderr("")
            .get_output()
            .stdout
            .clone();
        String::from_utf8(output).unwrap()
    }

    pub(crate) fn status(&self) -> String {
        let output =
            self.command().arg("status").assert().success().stderr("").get_output().stdout.clone();
        String::from_utf8(output).unwrap()
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.command().arg("stop").output();
    }
}
