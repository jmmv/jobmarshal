// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Top-level CLI contracts.

use crate::common::jobmarshal;
use std::fs;

const HELP: &str = r#"Usage: jobmarshal command [arg...]

Commands:
    add-wrapper         install a per-user command wrapper
    help                show command-line usage information
    list-wrappers       list per-user command wrappers
    remove-wrapper      remove a per-user command wrapper
    run                 run a command with the shared job budget
    start               start the per-user jobmarshal
    stats               inspect in-memory jobmarshal statistics
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

#[test]
fn test_stats_help_and_invalid_options() {
    let (mut command, directory) = jobmarshal();
    let output =
        command.args(["help", "stats"]).assert().success().stderr("").get_output().stdout.clone();
    assert!(String::from_utf8(output).unwrap().contains("--zero"));
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
    let (mut command, directory) = jobmarshal();
    command.args(["stats", "--invalid"]).assert().code(2).stdout("");
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_run_help_without_runtime_access() {
    let (mut command, directory) = jobmarshal();
    let output =
        command.args(["help", "run"]).assert().success().stderr("").get_output().stdout.clone();
    let help = String::from_utf8(output).unwrap();
    assert!(help.contains("COMMAND"));
    assert!(help.contains("ARG"));
    assert!(help.contains("optional --"));
    assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
}

#[test]
fn test_wrapper_help_without_storage_access() {
    for action in ["add-wrapper", "list-wrappers", "remove-wrapper"] {
        let (mut command, directory) = jobmarshal();
        command.args(["help", action]).assert().success().stderr("");
        assert_eq!(0, fs::read_dir(directory.path()).unwrap().count());
    }
}
