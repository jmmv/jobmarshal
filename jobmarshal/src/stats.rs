// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Human-readable statistics, separate from daemon accounting.

use jobmarshal_common::stats::Stats;
use std::io::{self, Write};
use std::time::SystemTime;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn timestamp(value: SystemTime) -> io::Result<String> {
    let nanos = match value.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => i128::try_from(duration.as_nanos()).map_err(io::Error::other)?,
        Err(error) => -i128::try_from(error.duration().as_nanos()).map_err(io::Error::other)?,
    };
    let date = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map_err(|error| io::Error::other(format!("invalid statistics timestamp: {error}")))?;
    date.format(&Rfc3339).map_err(io::Error::other)
}

pub(crate) fn print(writer: &mut dyn Write, stats: &Stats) -> io::Result<()> {
    let started = timestamp(stats.started)?;
    let reset = timestamp(stats.reset)?;
    for (label, value) in [
        ("Listening socket", stats.socket.display().to_string()),
        ("Token FIFO", stats.fifo.display().to_string()),
        ("Server PID", stats.pid.to_string()),
        ("Server started", started),
        ("Stats last reset", reset),
        ("Max jobs", stats.jobs.to_string()),
        ("Tokens available", stats.available.to_string()),
        ("Tokens unavailable", stats.unavailable.to_string()),
        ("Active builds", stats.active_builds.to_string()),
        ("Builds processed", stats.processed_builds.to_string()),
        ("Draining", stats.draining.to_string()),
    ] {
        writeln!(writer, "{label:<22}{value}")?;
    }
    writeln!(writer, "Token counts are a snapshot; job demand and job wait times are not measured.")
}

#[cfg(test)]
mod tests {
    use super::{print, timestamp};
    use jobmarshal_common::stats::Stats;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_timestamp_range_and_before_epoch() {
        assert_eq!(
            "1969-12-31T23:59:59Z",
            timestamp(SystemTime::UNIX_EPOCH - Duration::from_secs(1)).unwrap()
        );
        assert!(timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(253_402_300_800)).is_err());
    }

    #[test]
    fn test_print_labeled_utc_snapshot() {
        let stats = Stats {
            active_builds: 1,
            available: 2,
            draining: false,
            fifo: "/runtime/tokens".into(),
            jobs: 4,
            pid: 123,
            processed_builds: 5,
            reset: SystemTime::UNIX_EPOCH + Duration::new(2, 123_000_000),
            socket: "/runtime/control.sock".into(),
            started: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            unavailable: 2,
        };
        let mut output = Vec::new();
        print(&mut output, &stats).unwrap();
        assert_eq!(
            "Listening socket      /runtime/control.sock\nToken FIFO            /runtime/tokens\nServer PID            123\nServer started        1970-01-01T00:00:01Z\nStats last reset      1970-01-01T00:00:02.123Z\nMax jobs              4\nTokens available      2\nTokens unavailable    2\nActive builds         1\nBuilds processed      5\nDraining              false\nToken counts are a snapshot; job demand and job wait times are not measured.\n",
            String::from_utf8(output).unwrap()
        );
    }
}
