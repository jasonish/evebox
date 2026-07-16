// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Server-local packet capture service configuration and limits.

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::pcap::{PcapSource, SpoolConfig};
use crate::prelude::*;

/// Fixed serving limits and timeouts for packet capture requests.
#[derive(Debug, Clone)]
pub(crate) struct PcapSettings {
    /// Default per-request output size cap. A native GET may raise or
    /// lift it with its own `max_size`; buffered POST remains bounded
    /// by this value.
    pub(crate) max_bytes: u64,
    /// The first byte must arrive within this. Also doubles as the
    /// engine-side scan-time cap.
    pub(crate) request_timeout: Duration,
    /// Once a response is streaming, stop if the client does not drain
    /// output within this interval.
    pub(crate) stall_timeout: Duration,
    /// Backstop for extractions using the shared blocking pool.
    pub(crate) max_concurrent: usize,
    /// Grace period for a cancelled extraction to acknowledge the
    /// cancellation before its blocking task is detached.
    pub(crate) wedge_grace: Duration,
}

impl Default for PcapSettings {
    fn default() -> Self {
        Self {
            max_bytes: 8_000_000,
            request_timeout: Duration::from_secs(60),
            stall_timeout: Duration::from_secs(60),
            max_concurrent: 16,
            wedge_grace: Duration::from_secs(5),
        }
    }
}

/// The one server-local packet capture source together with its
/// extraction limits.
pub(crate) struct PcapService {
    pub(crate) settings: PcapSettings,
    source: Option<PcapSource>,
    global: Arc<Semaphore>,
    /// Extraction worker threads currently alive, including detached
    /// ones whose request was already answered.
    pub(crate) inflight: Arc<std::sync::atomic::AtomicUsize>,
}

impl Default for PcapService {
    fn default() -> Self {
        Self::new(PcapSettings::default(), None)
    }
}

impl PcapService {
    pub(crate) fn new(settings: PcapSettings, source: Option<PcapSource>) -> Self {
        let global = Arc::new(Semaphore::new(settings.max_concurrent));
        Self {
            settings,
            source,
            global,
            inflight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub(crate) fn source(&self) -> Option<&PcapSource> {
        self.source.as_ref()
    }

    pub(crate) fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// One global in-flight slot, or `None` when at capacity.
    pub(crate) fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.global.clone().try_acquire_owned().ok()
    }

    /// Test-only permit-release observability.
    #[cfg(test)]
    pub(crate) fn idle(&self) -> bool {
        self.global.available_permits() == self.settings.max_concurrent
    }
}

/// Parse a duration: a humantime string (`60s`, `5m`) or a bare
/// number of seconds.
pub(crate) fn parse_duration_seconds(input: &str) -> Result<std::time::Duration, String> {
    if let Ok(secs) = input.trim().parse::<u64>() {
        return Ok(std::time::Duration::from_secs(secs));
    }
    humantime::parse_duration(input).map_err(|err| err.to_string())
}

/// Build the server-local packet capture service from configuration.
pub(crate) fn configure(config: &crate::config::Config) -> PcapService {
    let directory = config
        .get::<String>("pcap.directory")
        .unwrap_or_else(|err| {
            warn!("Ignoring bad pcap.directory: {err}; pcap disabled");
            None
        });

    let spool = directory.map(|directory| {
        let directory = PathBuf::from(directory);
        if !directory.is_dir() {
            warn!(
                "Pcap spool directory {} does not exist (yet)",
                directory.display()
            );
        }
        let prefix = config.get::<String>("pcap.prefix").unwrap_or_else(|err| {
            warn!("Ignoring bad pcap.prefix: {err}");
            None
        });
        info!("Serving pcap from local spool {}", directory.display());
        SpoolConfig::new(directory, prefix)
    });

    PcapService::new(PcapSettings::default(), spool.map(PcapSource::Spool))
}

#[cfg(test)]
mod test {
    use super::*;

    /// A Config backed only by a YAML file (no CLI arguments).
    fn yaml_config(dir: &std::path::Path, yaml: &str) -> crate::config::Config {
        let path = dir.join("evebox.yaml");
        std::fs::write(&path, yaml).unwrap();
        let args = clap::Command::new("test").get_matches_from(["test"]);
        crate::config::Config::new(args, path.to_str()).unwrap()
    }

    #[test]
    fn test_configure_local_spool() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = format!("pcap:\n  directory: {}\n", dir.path().display());
        let config = yaml_config(dir.path(), &yaml);
        let service = configure(&config);
        assert!(service.has_source());
        assert_eq!(service.settings.max_bytes, 8_000_000);
        let Some(PcapSource::Spool(spool)) = service.source() else {
            panic!("expected a spool source");
        };
        assert_eq!(spool.directory, dir.path());
    }

    #[test]
    fn test_configure_without_spool() {
        let dir = tempfile::tempdir().unwrap();
        let config = yaml_config(dir.path(), "");
        assert!(!configure(&config).has_source());
    }
}
