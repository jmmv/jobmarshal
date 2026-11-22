// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Daemon-lifetime counters and observational statistics snapshots.

use crate::pool::Pool;
use anyhow::{Context, Result};
use jobmarshal_common::stats::Stats;
use std::path::PathBuf;
use std::time::SystemTime;

pub(crate) struct Accounting {
    processed: u64,
    reset: SystemTime,
    socket: PathBuf,
    started: SystemTime,
}

impl Accounting {
    pub(crate) fn new(socket: PathBuf, started: SystemTime) -> Self {
        Self { processed: 0, reset: started, socket, started }
    }

    pub(crate) fn complete(&mut self) {
        self.processed = self.processed.saturating_add(1);
    }

    pub(crate) fn reset(&mut self, now: SystemTime) {
        self.processed = 0;
        self.reset = now;
    }

    pub(crate) fn snapshot(&self, pool: &Pool, active: usize, draining: bool) -> Result<Stats> {
        let available = pool.available()?;
        let unavailable = pool
            .jobs
            .checked_sub(available)
            .context("token availability exceeds configured capacity")?;
        Ok(Stats {
            active_builds: active,
            available,
            draining,
            fifo: pool.fifo.clone(),
            jobs: pool.jobs,
            pid: std::process::id(),
            processed_builds: self.processed,
            reset: self.reset,
            socket: self.socket.clone(),
            started: self.started,
            unavailable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Accounting;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_accounting_completion_and_reset() {
        let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let reset = started + Duration::from_secs(2);
        let mut accounting = Accounting::new("/runtime/control.sock".into(), started);
        assert_eq!(0, accounting.processed);
        assert_eq!(started, accounting.reset);
        accounting.complete();
        accounting.complete();
        assert_eq!(2, accounting.processed);
        accounting.reset(reset);
        assert_eq!(0, accounting.processed);
        assert_eq!(reset, accounting.reset);
        assert_eq!(started, accounting.started);
        accounting.complete();
        assert_eq!(1, accounting.processed);
    }
}
