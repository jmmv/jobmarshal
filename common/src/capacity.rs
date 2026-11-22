// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Job capacity configuration.

use anyhow::{Context, Result, ensure};
use std::env;

/// Parses a strictly positive decimal capacity.
pub fn parse(value: &str) -> Result<usize> {
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "jobs must be a positive integer"
    );
    let jobs = value.parse().context("jobs must be a positive integer")?;
    ensure!(jobs > 0, "jobs must be positive");
    Ok(jobs)
}

/// Resolves an explicit CLI or environment request, without selecting a default.
pub fn requested(option: Option<String>) -> Result<Option<usize>> {
    let value = match option {
        Some(value) => Some(value),
        None => env::var_os("JOBMARSHAL_JOBS")
            .map(|value| value.into_string().map_err(|_| anyhow::anyhow!("jobs must be UTF-8")))
            .transpose()?,
    };
    value.as_deref().map(parse).transpose()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn test_parse_valid() {
        assert_eq!(1, parse("1").unwrap());
        assert_eq!(16, parse("016").unwrap());
    }

    #[test]
    fn test_parse_invalid() {
        for value in
            ["", "0", "-1", "+1", " 1", "1.0", "NaN", "999999999999999999999999999999999999"]
        {
            assert!(parse(value).is_err(), "{value}");
        }
    }
}
