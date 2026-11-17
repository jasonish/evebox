// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Retry delay for event commits that fail, typically because the
//! destination (EveBox server, Elasticsearch) is unreachable.

use std::time::Duration;

const MIN_DELAY: Duration = Duration::from_secs(1);

/// Upper bound on the time between retries. Events queue up behind a
/// failed commit, so don't wait long to notice the destination is back.
const MAX_DELAY: Duration = Duration::from_secs(60);

/// Exponential backoff starting at one second and doubling up to one
/// minute. Create a new one for each operation being retried.
pub(crate) struct RetryBackoff {
    next: Duration,
    attempts: u32,
}

impl RetryBackoff {
    pub(crate) fn new() -> Self {
        Self {
            next: MIN_DELAY,
            attempts: 0,
        }
    }

    /// Record a failure and return how long to wait before retrying.
    pub(crate) fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(MAX_DELAY);
        self.attempts += 1;
        delay
    }

    /// The number of failures recorded so far.
    pub(crate) fn attempts(&self) -> u32 {
        self.attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_doubles_and_never_exceeds_a_minute() {
        let mut backoff = RetryBackoff::new();
        let delays: Vec<u64> = (0..10).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 32, 60, 60, 60, 60]);
        assert_eq!(backoff.attempts(), 10);
    }
}
