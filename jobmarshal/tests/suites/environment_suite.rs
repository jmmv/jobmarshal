// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Session initialization without daemon or filesystem side effects.

use crate::common::jobmarshal;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn test_print_env_shell_roundtrip_and_idempotence() {
    let (mut command, root) = jobmarshal();
    let data = root.path().join("space and '\"$`\\data");
    let runtime = root.path().join("runtime'\"$`\\dir");
    let output = command
        .env("XDG_DATA_HOME", &data)
        .env("JOBMARSHAL_RUNTIME_DIR", &runtime)
        .env("PATH", "/bin:/usr/bin")
        .arg("print-env")
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let script = String::from_utf8(output).unwrap();
    let output = Command::new("sh")
        .args(["-c", &format!("{script}\nprintf '%s\\n%s' \"$JOBMARSHAL_RUNTIME_DIR\" \"$PATH\"")])
        .output()
        .unwrap();
    assert!(output.status.success());
    let path = format!("{}:/bin:/usr/bin", data.join("jobmarshal/bin").display());
    assert_eq!(format!("{}\n{path}", runtime.display()).as_bytes(), output.stdout);
    let (mut command, _) = jobmarshal();
    command
        .env("XDG_DATA_HOME", &data)
        .env("JOBMARSHAL_RUNTIME_DIR", &runtime)
        .env("PATH", path)
        .arg("print-env")
        .assert()
        .success()
        .stdout(script.lines().next().unwrap().to_owned() + "\n");
    assert_eq!(0, fs::read_dir(root.path()).unwrap().count());
}

#[test]
fn test_print_env_xdg_and_unsafe_existing_runtime() {
    let (mut command, root) = jobmarshal();
    let xdg = root.path().join("xdg");
    fs::create_dir(&xdg).unwrap();
    fs::set_permissions(&xdg, Permissions::from_mode(0o700)).unwrap();
    command
        .env_remove("JOBMARSHAL_RUNTIME_DIR")
        .env("XDG_RUNTIME_DIR", &xdg)
        .arg("print-env")
        .assert()
        .success()
        .stdout(format!(
            "export JOBMARSHAL_RUNTIME_DIR='{}'\nexport PATH='{}:{}'\n",
            xdg.join("jobmarshal").display(),
            root.path().join("data/jobmarshal/bin").display(),
            std::env::var("PATH").unwrap().replace('\'', "'\\''")
        ));
    assert_eq!(0, fs::read_dir(&xdg).unwrap().count());
    fs::write(root.path().join("runtime"), "untouched").unwrap();
    let (mut command, _) = jobmarshal();
    command
        .env("JOBMARSHAL_RUNTIME_DIR", root.path().join("runtime"))
        .arg("print-env")
        .assert()
        .success();
    assert_eq!("untouched", fs::read_to_string(root.path().join("runtime")).unwrap());
}
