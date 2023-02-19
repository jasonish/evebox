// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

/// Given a time range in seconds, return a suitable date histogram
/// interval.
pub fn histogram_interval(range: i64) -> u64 {
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
    } else {
        3600 * 24
    }
}
