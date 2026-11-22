// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Shared command fixtures.

use assert_cmd::Command;
use tempfile::{TempDir, tempdir};

pub(crate) fn jobmarshal() -> (Command, TempDir) {
    let directory = tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_jobmarshal"));
    command
        .env("HOME", directory.path().join("home"))
        .env("JOBMARSHAL_RUNTIME_DIR", directory.path().join("runtime"))
        .env("JOBMARSHAL_JOBS", "invalid")
        .env("XDG_RUNTIME_DIR", directory.path().join("xdg"));
    (command, directory)
}

pub(crate) mod service;
