// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Isolated daemon ownership for integration tests.

use assert_cmd::Command;
use std::path::PathBuf;
use std::process::Child;
use std::time::Duration;
use tempfile::{TempDir, tempdir};

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
        let mut command = Command::new(env!("CARGO_BIN_EXE_jobmarshal"));
        command
            .env("JOBMARSHAL_RUNTIME_DIR", &self.runtime)
            .env_remove("JOBMARSHAL_JOBS")
            .env("HOME", self.root.path())
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
