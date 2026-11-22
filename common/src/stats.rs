// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! In-memory service statistics exchanged over the control protocol.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// A point-in-time service observation; native token holders can change concurrently.
#[derive(Debug, Deserialize, Serialize)]
pub struct Stats {
    /// Number of admitted builds whose leases remain open.
    pub active_builds: usize,
    /// Bytes currently available in the native token FIFO.
    pub available: usize,
    /// Whether independent admissions are being rejected.
    pub draining: bool,
    /// Native token FIFO path.
    pub fifo: PathBuf,
    /// Maximum shared parallelism.
    pub jobs: usize,
    /// Daemon process identifier.
    pub pid: u32,
    /// Admitted leases completed since the last reset, including failed commands.
    pub processed_builds: u64,
    /// Time cumulative accounting was most recently reset.
    pub reset: SystemTime,
    /// Listening control socket path.
    pub socket: PathBuf,
    /// Time this daemon started.
    pub started: SystemTime,
    /// Capacity reserved, held by native clients, or lost by native clients.
    pub unavailable: usize,
}
