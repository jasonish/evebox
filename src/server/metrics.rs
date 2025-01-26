// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::{atomic::AtomicU64, Arc, Mutex};

use serde::Serialize;

use crate::sqlite::importer::SqliteEventConsumerMetrics;

#[derive(Debug, Default, Serialize)]
pub(crate) struct Metrics {
    pub start_time: crate::datetime::DateTime,
    pub autoarchived_by_age: AtomicU64,
    pub autoarchived_by_filter: AtomicU64,
    pub autoarchived_by_user: AtomicU64,
    pub sqlite_event_consumer: Arc<Mutex<SqliteEventConsumerMetrics>>,
    pub events_rx: AtomicU64,
}

impl Metrics {
    /// Increment the autoarchived_by_age count by n.
    pub fn incr_autoarchived_by_age(&self, n: u64) {
        self.autoarchived_by_age
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn incr_autoarchived_by_filter(&self, n: u64) {
        self.autoarchived_by_filter
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn incr_autoarchived_by_user(&self, n: u64) {
        self.autoarchived_by_user
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn incr_events_rx(&self, n: u64) {
        self.events_rx
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_metrics() {
        let a = Arc::new(Metrics::default());
        let b = a.clone();

        a.autoarchived_by_age
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            b.autoarchived_by_age
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        b.incr_autoarchived_by_age(1);
        assert_eq!(
            a.autoarchived_by_age
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }
}
