// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Top-level CLI contracts.

use crate::common::jobmarshal;
use std::fs;

const HELP: &str = r#"Usage: jobmarshal [options]

Options:
    -h, --help          show command-line usage information and exit
        --version       show version information and exit

Commands:
    help                show command-line usage information
    version             show version information

Jobmarshal home page: https://github.com/jmmv/jobmarshal
"#;

#[test]
fn test_help() {
    for argument in ["help", "--help", "-h"] {
        let (mut command, directory) = jobmarshal();
        command.arg(argument).assert().code(0).stdout(HELP).stderr("");
        assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
    }
}

#[test]
fn test_version() {
    for argument in ["version", "--version"] {
        let (mut command, directory) = jobmarshal();
        command
            .arg(argument)
            .assert()
            .code(0)
            .stdout(format!(
                "Jobmarshal {}\nCopyright 2026 Julio Merino\n",
                env!("CARGO_PKG_VERSION")
            ))
            .stderr("");
        assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
    }
}

#[test]
fn test_empty_arguments() {
    let (mut command, directory) = jobmarshal();
    command.assert().code(0).stdout("").stderr("");
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_unknown_command() {
    let (mut command, _directory) = jobmarshal();
    command.arg("unknown").assert().code(2).stdout("").stderr(
        "Usage error: Too many arguments\n\
            Type `jobmarshal --help` or `man 1 jobmarshal` for more information\n",
    );
}

#[test]
fn test_invalid_option() {
    let (mut command, _directory) = jobmarshal();
    command.arg("--invalid").assert().code(2).stdout("").stderr(
        "Usage error: Unrecognized option: 'invalid'\n\
            Type `jobmarshal --help` or `man 1 jobmarshal` for more information\n",
    );
}

#[test]
fn test_command_extra_arguments() {
    for command_name in ["help", "version"] {
        let (mut command, _directory) = jobmarshal();
        command.args([command_name, "--invalid"]).assert().code(2).stdout("").stderr(format!(
            "Usage error: {command_name:?} does not accept arguments\n\
                Type `jobmarshal help` or `man 1 jobmarshal` for more information\n"
        ));
    }
}
