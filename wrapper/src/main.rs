// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Transparent build wrapper executable.

fn main() {
    if let Err(error) = wrapper::execution::run() {
        eprintln!("jobmarshal-wrapper: {error:#}");
        std::process::exit(125);
    }
}
