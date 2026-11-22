# Jobmarshal

Jobmarshal is a per-user build coordination service for Linux and FreeBSD.
Independent wrapped builds share a native GNU make/Cargo job budget.

Homepage: <https://github.com/jmmv/jobmarshal>
Issues: <https://github.com/jmmv/jobmarshal/issues>

## Usage

```
eval "$(jobmarshal start -j 8)"
jobmarshal status
/usr/local/libexec/jobmarshal-wrapper cargo build
jobmarshal stats
jobmarshal stats --zero
jobmarshal stop
```

`start` detaches the daemon, writes diagnostics to a private `daemon.log`, and
prints a shell-quoted runtime-directory export.  Capacity comes from `--jobs`
(or `-j`), then `JOBMARSHAL_JOBS`, then available CPUs.  An existing daemon must
match an explicit capacity request; stop it before selecting another capacity.
`status` returns 0 when responsive and 1 when absent; `stop` fails when absent
and waits for complete teardown after active builds finish.  Queued and new
independent builds are rejected during draining; nested wrappers reuse their
inherited allocation and can finish without reserving another slot.
`jobmarshal help` and `jobmarshal version` do not access runtime configuration.

Runtime discovery uses `JOBMARSHAL_RUNTIME_DIR`, then the XDG runtime base plus
`jobmarshal`, then `/tmp/jobmarshal-<uid>` when no usable XDG runtime base
exists.  The selected directory must be absolute UTF-8 without whitespace, owned
by the user, and inaccessible to group/others.  Symlink directories and unsafe
lock or log files are rejected.  Stable lock files remain after shutdown.
Ordinary control connections and reads/writes have bounded deadlines.  Admission
and draining can wait as long as a build needs.

Each independent build reserves one token for its implicit slot; native GNU
make and Cargo clients use the remaining FIFO tokens.  The lease survives exec
and descendants, returning its token when its last descriptor closes.  Surviving
participants prevent replacement of a crashed daemon's pool.  Lost native tokens
reduce capacity conservatively until a safe stop and restart.

`jobmarshal stats` reports the socket, FIFO, PID, UTC RFC3339 start/reset times,
maximum jobs, available/unavailable tokens, active/processed builds, and
draining state.  It does not start an absent daemon.  `stats --zero` resets the
processed count and reset time before printing; active builds count when their
leases later end, including failed commands.  Queued requests that never receive
admission do not count.  Restart resets all in-memory statistics.

Token counts are snapshots collected without consuming tokens or changing native
descriptor flags.  Unavailable tokens include implicit build reservations, native
token holders, and lost tokens; they are not process or thread counts.  Job demand
and wait duration are not measured.

The installed `libexec/jobmarshal-wrapper COMMAND [ARG ...]` provides direct
execution.  Hardlinks with other basenames search PATH for their own inode and
execute the next real command, skipping wrapper aliases.  The management
`jobmarshal` must be at the wrapper's PATH anchor.  Direct invocation prefers a
sibling development `jobmarshal` and otherwise searches PATH.  Valid inherited
GNU/Cargo jobservers are reused; invalid advertised authentication fails with
status 125.  Flags without authentication are allowed.  All three native flag
variables retain unrelated options.  The native library selects the first
present variable from CARGO_MAKEFLAGS, MAKEFLAGS, and MFLAGS.

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

Installation respects `--bindir`, `--libexecdir`, `--docdir`, `--mandir`, and
other normal configure directory options.  Use `make install
DESTDIR=/path/to/stage` to stage an installation and `make uninstall` with the
same directory settings to remove it.  `make installcheck` verifies installation
and complete removal in a temporary stage.  The `jobmarshal(1)` and
`jobmarshal-wrapper(1)` manuals document behavior.

Configure selects the complete set of command wrapper hardlinks.  The default,
`--disable-wrappers`, and `--enable-wrappers=''` install no aliases.  For example:

```
../configure --prefix=/usr/local --enable-wrappers='cargo cc mold'
make install DESTDIR=/path/to/stage
```

Names must be basenames beginning with an ASCII letter or digit and containing
only ASCII letters, digits, `_`, `.`, `+`, or `-`.  `jobmarshal` and
`jobmarshal-wrapper` are reserved.  Names are deduplicated and sorted; bare
`--enable-wrappers` fails.  No installation-time name list or installed manifest
is used.  Put the installed bindir before the real commands in PATH to use the
wrappers.  Bindir and libexecdir must support hardlinks on the same filesystem;
installation checks this before replacing files and never substitutes symlinks.

Reinstalling with the same selection is safe.  Installation refuses unrelated
files or symlinks at configured alias destinations.  Uninstall preserves aliases
that have been replaced by unrelated files or symlinks.  Before changing the
selection, run `make uninstall` with the original configuration and directory
settings, then reconfigure and reinstall.  No stale aliases are inferred or
scanned.  Out-of-tree builds support spaces in the build directory; when the
source path contains spaces, use an in-tree build because Automake rejects an
explicit source-directory path containing spaces.

## Development

Run `cargo build --workspace --locked`, `cargo build --workspace --locked
--release`, and `cargo test --workspace --locked` for direct Cargo builds.
Always build the workspace binaries before running tests: integration tests
exercise both executables.  Tests use isolated runtime directories and daemon
cleanup guards, including real concurrent Cargo workspaces with synchronized
build-script activity.  Install prek 0.5.3, cargo-machete 0.9.2, ShellCheck, and
cargo-audit 0.22.2.  Run `prek run --all-files` (or `make lint`) and `cargo
audit -D warnings` before submitting changes.  `.cargo/audit.toml` records the
maintenance advisory exception for the required daemonize dependency.
`admin/check-sources.sh` verifies explicit distribution lists.  Render each
manual with `groff -Tutf8 -mdoc man/jobmarshal.1` (or `mandoc` on FreeBSD).
Linux and FreeBSD CI exercise builds, staged installation, and source archive
builds with `make distcheck`.  `make check` also validates configure selections,
hardlink ownership and reinstall behavior, unrelated replacements, changed
selections, paths with spaces, and cross-filesystem failure when a writable
second filesystem is available.

Copyright 2026 Julio Merino.  Licensed under GPL-3.0-only; see LICENSE.
