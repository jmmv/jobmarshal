// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Real Cargo processes share the aggregate native budget.

use crate::common::{ChildGuard, Service, fixture, wait, wrapper};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn workspace(root: &Path, id: &str) -> PathBuf {
    let directory = root.join(id);
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for member in ["a", "b"] {
        let package = directory.join(member);
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(package.join("Cargo.toml"), format!("[package]\nname = \"{id}-{member}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n")).unwrap();
        fs::write(package.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n").unwrap();
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build.rs"),
            package.join("build.rs"),
        )
        .unwrap();
    }
    directory
}

#[test]
fn test_real_cargo_workspaces_overlap_with_bounded_activity() {
    let service = Service::new();
    service.start("3");
    let activity = service.root.path().join("activity");
    fs::create_dir(&activity).unwrap();
    let mut children = Vec::new();
    for id in ["first", "second"] {
        let directory = workspace(service.root.path(), id);
        children.push(ChildGuard(
            wrapper(&service)
                .env("JOBMARSHAL_JOBS", "3")
                .arg("cargo")
                .arg("build")
                .arg("--offline")
                .current_dir(&directory)
                .env("CARGO_TARGET_DIR", directory.join("target"))
                .env("ACTIVITY_ROOT", &activity)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        ));
    }
    wait(|| {
        let names: Vec<_> = fs::read_dir(&activity)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.iter().any(|name| name.starts_with("first-") && name.ends_with(".active"))
            && names.iter().any(|name| name.starts_with("second-") && name.ends_with(".active"))
            && names.iter().filter(|name| name.ends_with(".active")).count() == 3
    });
    assert!(service.status().contains("active_builds=2"));
    let mut queued = ChildGuard(
        wrapper(&service)
            .env("JOBMARSHAL_JOBS", "3")
            .arg(fixture())
            .args(["exit", "0"])
            .spawn()
            .unwrap(),
    );
    wait(|| service.status().contains("active_builds=2 waiting_builds=1"));
    fs::write(activity.join("release"), "").unwrap();
    for child in &mut children {
        wait(|| child.0.try_wait().unwrap().is_some());
        assert!(child.0.wait().unwrap().success());
    }
    wait(|| queued.0.try_wait().unwrap().is_some());
    assert!(queued.0.wait().unwrap().success());
    let events = fs::read_to_string(activity.join("events")).unwrap();
    let counts: Vec<usize> = events.lines().map(|line| line.parse().unwrap()).collect();
    assert!(counts.iter().copied().max().unwrap() >= 2);
    assert!(counts.iter().all(|count| *count <= 3), "{events}");
    assert_eq!(0, counts.last().copied().unwrap());
}
