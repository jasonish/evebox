// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub(crate) mod pcap;

/// Given a time range in seconds, return a suitable date histogram
/// interval.
pub(crate) fn histogram_interval(range: i64) -> u64 {
    if range <= 60 {
        1
    } else if range <= 3600 {
        60
    } else if range <= 3600 * 3 {
        60 * 2
    } else if range <= 3600 * 6 {
        60 * 3
    } else if range <= 3600 * 12 {
        60 * 5
    } else if range <= 3600 * 24 {
        60 * 15
    } else if range <= 3600 * 24 * 3 {
        3600
    } else if range <= 3600 * 24 * 7 {
        3600 * 3
    } else if range <= 3600 * 24 * 14 {
        // 12 hours.
        3600 * 12
    } else {
        // 1 day.
        3600 * 24
    }
}

pub(crate) fn parse_humansize(input: &str) -> anyhow::Result<usize> {
    if let Ok(size) = input.parse::<usize>() {
        return Ok(size);
    }
    let re = regex::Regex::new(r"^(\d+)\s*(.*)$").unwrap();
    if let Some(matches) = re.captures(input) {
        let value = matches
            .get(1)
            .ok_or_else(|| anyhow::anyhow!("invalid size"))?
            .as_str();
        let value = value.parse::<usize>()?;
        let unit = matches
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("invalid size"))?
            .as_str();
        match unit {
            "GB" => value
                .checked_mul(1000000000)
                .ok_or_else(|| anyhow::anyhow!("size too large")),
            "MB" => value
                .checked_mul(1000000)
                .ok_or_else(|| anyhow::anyhow!("size too large")),
            _ => {
                bail!("invalid unit: {unit}")
            }
        }
    } else {
        bail!("invalid size: {input}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_humansize() {
        assert_eq!(parse_humansize("1").unwrap(), 1);
        assert_eq!(parse_humansize("1GB").unwrap(), 1000000000);
        assert_eq!(parse_humansize("100GB").unwrap(), 100000000000);
        assert_eq!(parse_humansize("1 GB").unwrap(), 1000000000);
        assert_eq!(parse_humansize("100 GB").unwrap(), 100000000000);
        assert_eq!(parse_humansize("1 MB").unwrap(), 1000000);

        assert!(parse_humansize("asdf").is_err());
        assert!(parse_humansize("1mb").is_err());

        // A unit multiply that would overflow usize is an error, not a
        // panic (debug) or silent wraparound (release).
        assert!(parse_humansize("20000000000GB").is_err());
    }
}
