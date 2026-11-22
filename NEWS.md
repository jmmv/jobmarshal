# Changes in version 0.1.0

- Coordinate transparent wrapped builds with FIFO tokens, inherited GNU/Cargo
  allocations, admission leases, draining, and crash-safe pool participation.
- Install `jobmarshal-wrapper` in libexec with its manual and validate real Cargo
  concurrency against the aggregate job budget.

- Establish the Cargo workspace, Autotools build, staged installation checks,
  source distributions, and Linux/FreeBSD CI.
- Add `jobmarshal help`, `jobmarshal version`, and the jobmarshal(1) manual.
- Add per-user start/status/stop commands, private runtime validation, stable
  lifecycle ownership, and a bounded versioned control protocol.
