// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Top-level CLI contracts.

use crate::common::jobmarshal;
use std::fs;

const HELP: &str = r#"Usage: jobmarshal command [arg...]

Commands:
    help                show command-line usage information
    start               start the per-user jobmarshal
    status              inspect the per-user jobmarshal
    stop                stop the per-user jobmarshal
    version             show version information

Jobmarshal home page: https://github.com/jmmv/jobmarshal
"#;

#[test]
fn test_help() {
    let (mut command, directory) = jobmarshal();
    command.arg("help").assert().code(0).stdout(HELP).stderr("");
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_version() {
    let (mut command, directory) = jobmarshal();
    command
        .arg("version")
        .assert()
        .code(0)
        .stdout(format!("Jobmarshal {}\nCopyright 2026 Julio Merino\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_empty_arguments() {
    let (mut command, directory) = jobmarshal();
    command.assert().code(2).stdout("").stderr(
        "Usage error: No command provided\n\
        Type `jobmarshal help` or `man 1 jobmarshal` for more information\n",
    );
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_unknown_command() {
    let (mut command, _directory) = jobmarshal();
    command.arg("unknown").assert().code(2).stdout("").stderr(
        "Usage error: Unknown command `unknown`\n\
        Type `jobmarshal help` or `man 1 jobmarshal` for more information\n",
    );
}

#[test]
fn test_invalid_option() {
    let (mut command, _directory) = jobmarshal();
    command.args(["start", "--invalid"]).assert().code(2).stdout("").stderr(
        "Usage error: Unrecognized option: 'invalid'\n\
        Type `jobmarshal help` or `man 1 jobmarshal` for more information\n",
    );
}

#[test]
fn test_help_unknown_command() {
    let (mut command, _directory) = jobmarshal();
    command.args(["help", "--invalid"]).assert().code(2).stdout("").stderr(
        "Usage error: Unknown command `--invalid`\n\
        Type `jobmarshal help` or `man 1 jobmarshal` for more information\n",
    );
}
