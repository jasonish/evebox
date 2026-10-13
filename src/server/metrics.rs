// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex, atomic::AtomicU64};

use serde::Serialize;

use crate::sqlite::importer::SqliteEventConsumerMetrics;

/// The `/api/metrics` response: the maintained counters plus gauges that are
/// computed from live state at read time.
#[derive(Serialize)]
pub(crate) struct MetricsSnapshot {
    #[serde(flatten)]
    pub metrics: Arc<Metrics>,
    /// Read directly from the agent registry so same-name replacement and
    /// generation-safe removal cannot make a maintained gauge drift.
    pub agents_connected: usize,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Metrics {
    pub start_time: crate::datetime::DateTime,
    pub autoarchived_by_age: AtomicU64,
    pub autoarchived_by_filter: AtomicU64,
    pub autoarchived_by_user: AtomicU64,
    pub sqlite_event_consumer: Arc<Mutex<SqliteEventConsumerMetrics>>,
    /// Events submitted by agents.
    pub events_rx: AtomicU64,
    /// Events read by the server's own EVE file and socket inputs.
    pub events_input: AtomicU64,
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

    pub fn incr_events_input(&self, n: u64) {
        self.events_input
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }

    /// Total events that have passed through this server from any
    /// source: agent submissions plus local file and socket inputs.
    pub fn events_ingested(&self) -> u64 {
        self.events_rx.load(std::sync::atomic::Ordering::Relaxed)
            + self.events_input.load(std::sync::atomic::Ordering::Relaxed)
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

    #[test]
    fn events_ingested_sums_all_sources() {
        let metrics = Metrics::default();
        assert_eq!(metrics.events_ingested(), 0);
        metrics.incr_events_rx(3);
        assert_eq!(metrics.events_ingested(), 3);
        metrics.incr_events_input(2);
        assert_eq!(metrics.events_ingested(), 5);
    }
}
