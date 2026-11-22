# Changes in version 0.1.0

- Add optional systemd user service, environment generator, and profile.d
  initialization gated on service enablement, selected by configure detection
  or `--with-systemd`.  Add portable `serve` and `print-env` commands, including
  shell and systemd environment formats independent of installation settings.

- Prefer per-user `add-wrapper`, `list-wrappers`, and `remove-wrapper` commands
  with XDG data storage, a private wrapper copy, automatic refresh on new daemon
  launches, and idempotent PATH exports from `start`.

- Add `jobmarshal run [--] COMMAND [ARG ...]` with transparent OS-string command
  execution, shared admission, and a relocatable bin/libexec installation layout.
- Add `jobmarshal stats` and `stats --zero` with observational native token counts,
  UTC timestamps, completed-build accounting, and in-memory reset semantics.
- Select wrapper hardlinks with `configure --enable-wrappers='cargo cc mold'`,
  including safe reinstall, protected unrelated paths, and staged packaging.
- Coordinate transparent wrapped builds with FIFO tokens, inherited GNU/Cargo
  allocations, admission leases, draining, and crash-safe pool participation.
- Install `jobmarshal-wrapper` in libexec with its manual and validate real Cargo
  concurrency against the aggregate job budget.

- Establish the Cargo workspace, Autotools build, staged installation checks,
  source distributions, and Linux/FreeBSD CI.
- Add `jobmarshal help`, `jobmarshal version`, and the jobmarshal(1) manual.
- Add per-user start/status/stop commands, private runtime validation, stable
  lifecycle ownership, and a bounded versioned control protocol.
