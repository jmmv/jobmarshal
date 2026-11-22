// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Standalone native-token and process-behavior fixture.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

fn hold(path: &Path) {
    fs::write(path.with_extension("ready"), std::process::id().to_string()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.with_extension("release").exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
}

fn main() {
    let mut args = env::args_os().skip(1);
    match args.next().unwrap().to_str().unwrap() {
        "arg0" => {
            for argument in env::args_os().take(1).chain(args) {
                io::stdout().write_all(argument.as_encoded_bytes()).unwrap();
                io::stdout().write_all(b"\n").unwrap();
            }
        }
        "hold" => hold(Path::new(&args.next().unwrap())),
        "native" | "lose" => {
            let lose = env::args().nth(1).unwrap() == "lose";
            let flags = env::var("CARGO_MAKEFLAGS").unwrap();
            let path =
                flags.rsplit_once("--jobserver-auth=fifo:").unwrap().1.split(' ').next().unwrap();
            let mut fifo = OpenOptions::new().read(true).write(true).open(path).unwrap();
            let mut byte = [0];
            fifo.read_exact(&mut byte).unwrap();
            hold(Path::new(&args.next().unwrap()));
            if !lose {
                fifo.write_all(&byte).unwrap();
            }
        }
        "forward" => {
            for argument in args {
                io::stdout().write_all(argument.as_encoded_bytes()).unwrap();
                io::stdout().write_all(b"\0").unwrap();
            }
            println!("cwd={}", env::current_dir().unwrap().display());
            println!("env={}", env::var("FIXTURE_VALUE").unwrap());
            for name in ["CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"] {
                println!("{name}={}", env::var(name).unwrap());
            }
            let mut stdin = Vec::new();
            io::stdin().read_to_end(&mut stdin).unwrap();
            io::stdout().write_all(&stdin).unwrap();
            eprintln!("fixture stderr");
        }
        "nested" => {
            let status = Command::new(args.next().unwrap()).args(args).status().unwrap();
            std::process::exit(status.code().unwrap_or(99));
        }
        "nested-gated" => {
            hold(Path::new(&args.next().unwrap()));
            let status = Command::new(args.next().unwrap()).args(args).status().unwrap();
            std::process::exit(status.code().unwrap_or(99));
        }
        "descendant" => {
            Command::new(env::current_exe().unwrap()).arg("hold").args(args).spawn().unwrap();
        }
        "exit" => std::process::exit(args.next().unwrap().to_str().unwrap().parse().unwrap()),
        _ => panic!("unknown fixture mode"),
    }
}
