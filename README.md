# Jobmarshal

Jobmarshal is a per-user build coordination project for Linux and FreeBSD.
The `jobmarshal help` and `jobmarshal version` commands display command-line
usage and version information.  With no arguments, `jobmarshal` exits
successfully without output.  See jobmarshal(1) for details.

Homepage: <https://github.com/jmmv/jobmarshal>
Issues: <https://github.com/jmmv/jobmarshal/issues>

## Build and install

Install Rust 1.100 through rustup, Autoconf 2.69 or newer, Automake 1.16
or newer, and groff to render the manual.  On FreeBSD use GNU make (`gmake`)
for the commands below.

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
it.  `make installcheck` verifies installation and complete removal in a temporary
stage.

## Development

Run `cargo build --workspace --locked`, `cargo build --workspace --locked
--release`, and `cargo test --workspace --locked` for direct Cargo builds.
Install prek 0.5.3, cargo-machete 0.9.2, ShellCheck, and cargo-audit 0.22.2.
Run `prek run --all-files` (or `make lint`) and `cargo audit -D warnings` before
submitting changes.  `admin/check-sources.sh` verifies the explicit distribution
lists.  Linux and FreeBSD CI exercise builds, staged installation, and source
archive builds with `make distcheck`.

Copyright 2026 Julio Merino.  Licensed under GPL-3.0-only; see LICENSE.
