# Jobmarshal

Jobmarshal is a per-user build coordination service for Linux and FreeBSD.
It currently provides lifecycle commands and a bounded private control protocol.

Homepage: <https://github.com/jmmv/jobmarshal>
Issues: <https://github.com/jmmv/jobmarshal/issues>

## Usage

```
eval "$(jobmarshal start -j 8)"
jobmarshal status
jobmarshal stop
```

`start` detaches the daemon, writes diagnostics to a private `daemon.log`, and
prints a shell-quoted runtime-directory export.  Capacity comes from `--jobs`
(or `-j`), then `JOBMARSHAL_JOBS`, then available CPUs.  An existing daemon must
match an explicit capacity request; stop it before selecting another capacity.
`status` returns 0 when responsive and 1 when absent; `stop` fails when absent
and waits for complete teardown.  Active and waiting counts are currently zero.
`jobmarshal help` and `jobmarshal version` do not access runtime configuration.

Runtime discovery uses `JOBMARSHAL_RUNTIME_DIR`, then the XDG runtime base plus
`jobmarshal`, then `/tmp/jobmarshal-<uid>` when no usable XDG runtime base
exists.  The selected directory must be absolute UTF-8 without whitespace, owned
by the user, and inaccessible to group/others.  Symlink directories and unsafe
lock or log files are rejected.  Stable lock files remain after shutdown.
Ordinary control connections and reads/writes have bounded deadlines.

## Build and install

Install Rust 1.100 through rustup, Autoconf 2.69 or newer, and Automake 1.16
or newer.  On FreeBSD use GNU make (`gmake`) for the commands below.

```
autoreconf -is
mkdir build
cd build
../configure --prefix=/usr/local
make -j4
make check
make installcheck
make distcheck
make install
```

`make debug` builds the debug executable and `make release` builds the release
executable.  Cargo uses a build-local target directory, including when building
outside the source tree.  `make clean` removes Cargo outputs.

Installation respects `--bindir`, `--docdir`, `--mandir`, and other normal
configure directory options.  Use `make install DESTDIR=/path/to/stage` to stage
an installation and `make uninstall` with the same directory settings to remove
it.  `make installcheck` verifies installation and complete removal in a
temporary stage.  The `jobmarshal(1)` manual documents current commands.

## Development

Run `cargo build --workspace --locked`, `cargo build --workspace --locked
--release`, and `cargo test --workspace --locked` for direct Cargo builds.
Integration tests use isolated runtime directories and daemon cleanup guards.
Install prek 0.5.3, cargo-machete 0.9.2, ShellCheck, and cargo-audit 0.22.2.  Run
`prek run --all-files` (or `make lint`) and `cargo audit -D warnings` before
submitting changes.  `.cargo/audit.toml` records the maintenance advisory
exception for the required daemonize dependency.  `admin/check-sources.sh`
verifies explicit distribution lists.  Render the manual with `groff -Tutf8
-mdoc man/jobmarshal.1` (or `mandoc` on FreeBSD).  Linux and FreeBSD CI exercise
builds, staged installation, and source archive builds with `make distcheck`.

Copyright 2026 Julio Merino.  Licensed under GPL-3.0-only; see LICENSE.
