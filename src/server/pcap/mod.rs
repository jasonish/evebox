// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Packet-capture source routing, limits, and remote task coordination.

pub(crate) mod tasks;

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::pcap::{PcapSource, SpoolConfig};
use crate::prelude::*;
use crate::server::agents::{AgentEntry, AgentRegistry, LOCAL_PCAP_SOURCE_NAME};

/// A source selected for one normalized capture request.
pub(crate) enum ResolvedPcapSource {
    Local {
        name: String,
        // Consumed by the local extraction path, which Windows omits.
        #[cfg_attr(windows, allow(dead_code))]
        source: PcapSource,
        busy: Arc<Semaphore>,
    },
    Agent(Arc<AgentEntry>),
}

impl ResolvedPcapSource {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Local { name, .. } => name,
            Self::Agent(entry) => &entry.name,
        }
    }

    /// One in-flight extraction slot per source: the local spool serializes
    /// its disk work just like each remote agent serializes its own.
    pub(crate) fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        let busy = match self {
            Self::Local { busy, .. } => busy,
            Self::Agent(entry) => &entry.pcap_busy,
        };
        busy.clone().try_acquire_owned().ok()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RouteError {
    NoSource(Option<String>),
    Ambiguous(Vec<String>),
}

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
    /// cancellation before its blocking task is detached. Read by the
    /// local extraction supervisor, which Windows omits.
    #[cfg_attr(windows, allow(dead_code))]
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

/// The one server-local packet capture source together with its extraction limits.
pub(crate) struct PcapService {
    pub(crate) settings: PcapSettings,
    source: Option<PcapSource>,
    global: Arc<Semaphore>,
    local_busy: Arc<Semaphore>,
    /// Extraction worker threads currently alive, including detached
    /// ones whose request was already answered. Read by the local
    /// extraction path, which Windows omits.
    #[cfg_attr(windows, allow(dead_code))]
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
            local_busy: Arc::new(Semaphore::new(1)),
            inflight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
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

    /// The local capture input as a resolvable source, when one is configured.
    /// It always carries the reserved name `(server)` rather than a sensor
    /// identity of its own.
    fn local_source(&self) -> Option<ResolvedPcapSource> {
        self.source.clone().map(|source| ResolvedPcapSource::Local {
            name: LOCAL_PCAP_SOURCE_NAME.to_string(),
            source,
            busy: self.local_busy.clone(),
        })
    }

    /// Resolve a request across the optional server-local spool and live
    /// agents advertising the `pcap` capability.
    ///
    /// Agents are matched by the event's sensor identity, then its EveBox
    /// agent identifier stamp, then the older hostname stamp. The local
    /// spool has no identity to match: it serves
    /// explicit `(server)` requests and events with no agent stamp — those
    /// were ingested by this server's own input, so the local Suricata spool
    /// holds their packets whatever their sensor identity says. Stamped
    /// events whose agent is gone are never quietly served from the local
    /// spool.
    pub(crate) fn resolve_source(
        &self,
        agents: &AgentRegistry,
        event: Option<&serde_json::Value>,
        explicit: Option<&str>,
    ) -> Result<ResolvedPcapSource, RouteError> {
        if let Some(name) = explicit {
            if name == LOCAL_PCAP_SOURCE_NAME {
                return self
                    .local_source()
                    .ok_or_else(|| RouteError::NoSource(Some(name.to_string())));
            }
            return agents
                .pcap_agent(name)
                .map(ResolvedPcapSource::Agent)
                .ok_or_else(|| RouteError::NoSource(Some(name.to_string())));
        }

        let identity = event.and_then(sensor_identity);
        if let Some(name) = identity
            && let Some(entry) = agents.pcap_agent(name)
        {
            return Ok(ResolvedPcapSource::Agent(entry));
        }

        // The importer stamp is exact: an event carrying `evebox.agent.id`
        // was imported by that agent, so only its spool can serve the event.
        // The fuzzier hostname stamp remains for events imported before the
        // identifier stamp existed.
        if let Some(id) = event.and_then(stamped_agent_id) {
            return agents
                .pcap_agent(id)
                .map(ResolvedPcapSource::Agent)
                .ok_or_else(|| RouteError::NoSource(Some(id.to_string())));
        }

        if let Some(hostname) = event.and_then(agent_hostname) {
            let mut matches: Vec<ResolvedPcapSource> = agents
                .pcap_agents()
                .into_iter()
                .filter(|entry| entry.hostname == hostname)
                .map(ResolvedPcapSource::Agent)
                .collect();
            return match matches.len() {
                0 => Err(RouteError::NoSource(
                    identity.or(Some(hostname)).map(ToOwned::to_owned),
                )),
                1 => Ok(matches.pop().expect("one source")),
                _ => {
                    let mut names: Vec<String> = matches
                        .iter()
                        .map(|source| source.name().to_string())
                        .collect();
                    names.sort();
                    Err(RouteError::Ambiguous(names))
                }
            };
        }

        // An unstamped event came in through this server's own input.
        if event.is_some() {
            if let Some(local) = self.local_source() {
                return Ok(local);
            }
            if let Some(name) = identity {
                return Err(RouteError::NoSource(Some(name.to_string())));
            }
        }

        // A standalone request, or an anonymous event with no local spool:
        // a single available source serves it; more than one must be chosen
        // explicitly.
        let mut sources = Vec::new();
        if let Some(local) = self.local_source() {
            sources.push(local);
        }
        sources.extend(
            agents
                .pcap_agents()
                .into_iter()
                .map(ResolvedPcapSource::Agent),
        );
        match sources.len() {
            0 => Err(RouteError::NoSource(None)),
            1 => Ok(sources.pop().expect("one source")),
            _ => {
                let mut names: Vec<String> = sources
                    .iter()
                    .map(|source| source.name().to_string())
                    .collect();
                names.sort();
                Err(RouteError::Ambiguous(names))
            }
        }
    }

    /// Test-only permit-release observability. The consuming suites
    /// exercise extraction, so Windows compiles them out.
    #[cfg(all(test, not(windows)))]
    pub(crate) fn idle(&self) -> bool {
        self.global.available_permits() == self.settings.max_concurrent
    }
}

/// Normalized sensor identity for plain EVE and ECS-shaped events.
///
/// Plain EVE and legacy Elastic carry `host` as a string. ECS carries `host`
/// as an object, and EveBox keys the sensor on `agent.name` there (mirroring
/// `map_field("host")` and `get_sensors`), so `agent.name` must win over the
/// OS hostname in `host.name`; otherwise an ECS event whose `host.name`
/// differs from `agent.name` never matches its configured source.
pub(crate) fn sensor_identity(source: &serde_json::Value) -> Option<&str> {
    source["host"]
        .as_str()
        .or_else(|| source["agent"]["name"].as_str())
        .or_else(|| source["host"]["name"].as_str())
}

/// Agent identifier stamped by an EveBox agent on an imported event; the
/// exact name that agent claims on the control channel.
pub(crate) fn stamped_agent_id(source: &serde_json::Value) -> Option<&str> {
    source["evebox"]["agent"]["id"].as_str()
}

/// Hostname stamped by an EveBox agent on an imported event.
pub(crate) fn agent_hostname(source: &serde_json::Value) -> Option<&str> {
    source["evebox"]["agent"]["hostname"].as_str()
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
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Local extraction needs libpcap, which Windows builds omit; agents
    // remain the capture sources there.
    #[cfg(windows)]
    let directory = directory.and_then(|directory| -> Option<String> {
        warn!(
            "Ignoring pcap.directory {directory:?}: server-local pcap capture is not \
             supported on Windows; run an EveBox agent on this host to serve its spool"
        );
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
        let prefix = config
            .get::<String>("pcap.prefix")
            .unwrap_or_else(|err| {
                warn!("Ignoring bad pcap.prefix: {err}");
                None
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        info!("Serving pcap from local spool {}", directory.display());
        SpoolConfig::new(directory, prefix)
    });

    PcapService::new(PcapSettings::default(), spool.map(PcapSource::Spool))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::agent::protocol::{AgentHandshake, CAPABILITY_PCAP};

    fn register_agent(registry: &AgentRegistry, name: &str, hostname: &str) -> Arc<AgentEntry> {
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        registry.register(
            AgentHandshake {
                name: name.to_string(),
                hostname: hostname.to_string(),
                version: "test".to_string(),
                capabilities: vec![CAPABILITY_PCAP.to_string()],
            },
            tx,
        )
    }

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

    #[test]
    fn test_configure_normalizes_blank_and_padded_strings() {
        let dir = tempfile::tempdir().unwrap();
        let config = yaml_config(
            dir.path(),
            "pcap:\n  directory: '   '\n  prefix: ' ignored '\n",
        );
        let service = configure(&config);
        assert!(!service.has_source());

        let spool = dir.path().join("spool");
        std::fs::create_dir(&spool).unwrap();
        let yaml = format!(
            "pcap:\n  directory: '  {}  '\n  prefix: '  log.pcap  '\n",
            spool.display()
        );
        let service = configure(&yaml_config(dir.path(), &yaml));
        let Some(PcapSource::Spool(source)) = service.source() else {
            panic!("expected a spool source");
        };
        assert_eq!(source.directory, spool);
        assert_eq!(source.prefix.as_deref(), Some("log.pcap"));
    }

    #[test]
    fn resolver_prefers_explicit_then_event_identity() {
        let dir = tempfile::tempdir().unwrap();
        let service = PcapService::new(
            PcapSettings::default(),
            Some(PcapSource::Spool(SpoolConfig::new(dir.path(), None))),
        );
        let agents = AgentRegistry::default();
        register_agent(&agents, "remote", "remote-host");
        let remote_event = serde_json::json!({ "host": "remote" });

        // Explicit selection beats the event's identity, and the reserved
        // name always selects the local spool.
        assert!(matches!(
            service
                .resolve_source(&agents, Some(&remote_event), Some(LOCAL_PCAP_SOURCE_NAME))
                .unwrap(),
            ResolvedPcapSource::Local { name, .. } if name == LOCAL_PCAP_SOURCE_NAME
        ));
        assert!(matches!(
            service
                .resolve_source(&agents, Some(&remote_event), None)
                .unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "remote"
        ));
        assert!(matches!(
            PcapService::default().resolve_source(&agents, None, Some(LOCAL_PCAP_SOURCE_NAME)),
            Err(RouteError::NoSource(Some(name))) if name == LOCAL_PCAP_SOURCE_NAME
        ));
    }

    #[test]
    fn resolver_serves_unstamped_events_from_local_spool() {
        // An event without an EveBox agent-hostname stamp was ingested by
        // this server, so the local spool serves it no matter what its
        // sensor identity says.
        let dir = tempfile::tempdir().unwrap();
        let service = PcapService::new(
            PcapSettings::default(),
            Some(PcapSource::Spool(SpoolConfig::new(dir.path(), None))),
        );
        let agents = AgentRegistry::default();
        register_agent(&agents, "remote", "remote-host");
        let unmatched = serde_json::json!({ "host": "unmatched-sensor" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&unmatched), None).unwrap(),
            ResolvedPcapSource::Local { name, .. } if name == LOCAL_PCAP_SOURCE_NAME
        ));
    }

    #[test]
    fn resolver_uses_exact_agent_id_stamp_over_shared_hostname() {
        // Two agents on one host: the hostname stamp alone is ambiguous, but
        // the importing agent's identifier stamp routes exactly.
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "suri-8", "shared-host");
        register_agent(&agents, "suri-9", "shared-host");

        let stamped = serde_json::json!({
            "evebox": { "agent": { "id": "suri-9", "hostname": "shared-host" } }
        });
        assert!(matches!(
            service.resolve_source(&agents, Some(&stamped), None).unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "suri-9"
        ));

        // Without the id stamp the hostname is honestly ambiguous.
        let hostname_only = serde_json::json!({
            "evebox": { "agent": { "hostname": "shared-host" } }
        });
        assert!(matches!(
            service.resolve_source(&agents, Some(&hostname_only), None),
            Err(RouteError::Ambiguous(candidates))
                if candidates == ["suri-8".to_string(), "suri-9".to_string()]
        ));

        // The id stamp is authoritative: when its agent is gone the event is
        // not re-routed by the (matching) hostname stamp.
        let orphaned = serde_json::json!({
            "evebox": { "agent": { "id": "gone", "hostname": "shared-host" } }
        });
        assert!(matches!(
            service.resolve_source(&agents, Some(&orphaned), None),
            Err(RouteError::NoSource(Some(name))) if name == "gone"
        ));
    }

    #[test]
    fn resolver_uses_agent_hostname_but_never_falls_through_stamped_events() {
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "remote", "remote-host");
        let stamped = serde_json::json!({
            "host": "unmatched-sensor",
            "evebox": { "agent": { "hostname": "remote-host" } }
        });
        assert!(matches!(
            service.resolve_source(&agents, Some(&stamped), None).unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "remote"
        ));

        // A stamped event whose importer is gone is never served from the
        // local spool, even when one is configured.
        let dir = tempfile::tempdir().unwrap();
        let spooled = PcapService::new(
            PcapSettings::default(),
            Some(PcapSource::Spool(SpoolConfig::new(dir.path(), None))),
        );
        let orphaned = serde_json::json!({
            "host": "unmatched-sensor",
            "evebox": { "agent": { "hostname": "gone-host" } }
        });
        assert!(matches!(
            spooled.resolve_source(&agents, Some(&orphaned), None),
            Err(RouteError::NoSource(Some(name))) if name == "unmatched-sensor"
        ));

        // Without a local spool, an unstamped, unmatched event has no source.
        let unmatched = serde_json::json!({ "host": "unmatched-sensor" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&unmatched), None),
            Err(RouteError::NoSource(Some(name))) if name == "unmatched-sensor"
        ));
    }

    #[test]
    fn sensor_identity_prefers_agent_name_over_ecs_host_name() {
        // Plain EVE / legacy Elastic: string host.
        assert_eq!(
            sensor_identity(&serde_json::json!({ "host": "sensor1" })),
            Some("sensor1")
        );
        // ECS: host is an object holding the OS hostname; agent.name is the
        // canonical sensor identity (map_field("host") == "agent.name") and
        // must win over host.name.
        assert_eq!(
            sensor_identity(&serde_json::json!({
                "host": { "name": "web01.corp" },
                "agent": { "name": "fw-east" }
            })),
            Some("fw-east")
        );
        // ECS without agent.name still falls back to host.name.
        assert_eq!(
            sensor_identity(&serde_json::json!({ "host": { "name": "web01.corp" } })),
            Some("web01.corp")
        );
        assert_eq!(sensor_identity(&serde_json::json!({})), None);
    }

    #[test]
    fn resolver_routes_ecs_events_by_agent_name() {
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "sensor-host");
        // An ECS event: host.name is the OS hostname while agent.name is the
        // sensor EveBox displays and the operator names the agent after. The
        // request carries no explicit source and no EveBox agent-hostname
        // stamp, so routing must key on agent.name to reach the live agent.
        let ecs_event = serde_json::json!({
            "host": { "name": "web01.corp" },
            "agent": { "name": "fw-east" }
        });
        assert!(matches!(
            service
                .resolve_source(&agents, Some(&ecs_event), None)
                .unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "fw-east"
        ));
    }

    #[test]
    fn resolver_requires_source_when_standalone_request_is_ambiguous() {
        let dir = tempfile::tempdir().unwrap();
        let service = PcapService::new(
            PcapSettings::default(),
            Some(PcapSource::Spool(SpoolConfig::new(dir.path(), None))),
        );
        let agents = AgentRegistry::default();
        register_agent(&agents, "remote", "remote-host");
        assert!(matches!(
            service.resolve_source(&agents, None, None),
            Err(RouteError::Ambiguous(candidates))
                if candidates == [LOCAL_PCAP_SOURCE_NAME.to_string(), "remote".to_string()]
        ));
    }
}
