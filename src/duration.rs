// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Shared duration syntax for API time ranges and PCAP windows.

use crate::prelude::*;
use fundu::{DurationParser, TimeUnit};
use std::time::Duration;

/// Parse non-negative integer seconds, or integer components with s/m/h/d/w
/// suffixes, optionally separated by whitespace (e.g. `1h30m`, `1h 30m`).
/// Bare integers are seconds. Calendar months and years are not supported.
pub(crate) fn parse(input: &str) -> Result<Duration> {
    const PARSER: DurationParser<'static> = DurationParser::builder()
        .time_units(&[
            TimeUnit::Second,
            TimeUnit::Minute,
            TimeUnit::Hour,
            TimeUnit::Day,
            TimeUnit::Week,
        ])
        .parse_multiple(None)
        .disable_fraction()
        .disable_exponent()
        .disable_infinity()
        .build();
    let duration: Duration = PARSER.parse(input.trim())?.try_into()?;
    // Fundu saturates on overflow. Keep accepted durations within the signed
    // timestamp arithmetic range, rejecting saturation rather than accepting it.
    time::Duration::try_from(duration).map_err(|_| anyhow!("duration out of range"))?;
    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_and_compound_durations() {
        for (input, seconds) in [
            ("1m", 60),
            ("5m", 300),
            ("15m", 900),
            ("1h", 3600),
            ("3h", 10800),
            ("12h", 43200),
            ("24h", 86400),
            ("1d", 86400),
            ("3d", 259200),
            ("7d", 604800),
            ("1w", 604800),
            ("30s", 30),
            ("60", 60),
            (" 60 ", 60),
            ("1h30m", 5400),
            ("1h 30m", 5400),
            ("0s", 0),
        ] {
            assert_eq!(
                parse(input).unwrap(),
                Duration::from_secs(seconds),
                "{input}"
            );
        }
    }

    #[test]
    fn unsupported_and_oversized_inputs() {
        for input in [
            "",
            "-1s",
            "1h -30m",
            "1.5h",
            "1e3s",
            "inf",
            "infinity",
            "1M",
            "1y",
            "2 hours",
            "1ms",
            "garbage",
            "18446744073709551616s",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}
