// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Packet-capture source routing, limits, and remote task coordination.

pub(crate) mod tasks;

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::agent::protocol::CAPABILITY_PCAP;
use crate::pcap::{PcapSource, SpoolConfig};
use crate::prelude::*;
use crate::server::agents::{AgentEntry, AgentRegistry, LOCAL_PCAP_SOURCE_NAME};
use crate::server::routing::{self, Resolved};

/// A source selected for one normalized capture request.
pub(crate) enum ResolvedPcapSource {
    Local {
        name: String,
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

pub(crate) use crate::server::routing::RouteError;

/// The operator-controlled routing table: ordered sensor to source
/// rules with an optional default source. Persisted in the configdb kv
/// table under `config.pcap.routing`. When present (at least one rule
/// or a default), routing is fully operator-controlled and the
/// implicit heuristics never run — including the agent identifier
/// stamp, which routes to the *importing* agent while the whole point
/// of a rule is to say some other source holds the sensor's packets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PcapRouting {
    pub(crate) rules: Vec<PcapRoutingRule>,
    pub(crate) default: Option<String>,
}

/// One routing rule: an exact-match sensor identity mapped to a source
/// name. First match wins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PcapRoutingRule {
    pub(crate) sensor: String,
    pub(crate) source: String,
}

impl PcapRouting {
    /// A table with no rules and no default is absent: the implicit
    /// routing heuristics apply.
    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.default.is_none()
    }
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
    /// How long the pre-dispatch liveness probe waits for a remote agent
    /// to answer a WebSocket ping before the request is refused.
    pub(crate) liveness_timeout: Duration,
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
            liveness_timeout: Duration::from_secs(2),
            max_concurrent: 16,
            wedge_grace: Duration::from_secs(5),
        }
    }
}

/// The one server-local packet capture source together with its extraction limits.
pub(crate) struct PcapService {
    pub(crate) settings: PcapSettings,
    source: Option<PcapSource>,
    routing: std::sync::RwLock<PcapRouting>,
    /// Held across a routing-table save (configdb write then
    /// `set_routing`) so concurrent saves cannot interleave and leave
    /// the persisted and live tables silently diverged.
    pub(crate) routing_save: tokio::sync::Mutex<()>,
    global: Arc<Semaphore>,
    local_busy: Arc<Semaphore>,
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
            routing: std::sync::RwLock::new(PcapRouting::default()),
            routing_save: tokio::sync::Mutex::new(()),
            global,
            local_busy: Arc::new(Semaphore::new(1)),
            inflight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Replace the operator routing table.
    pub(crate) fn set_routing(&self, routing: PcapRouting) {
        *self.routing.write().unwrap() = routing;
    }

    /// The current operator routing table.
    pub(crate) fn get_routing(&self) -> PcapRouting {
        self.routing.read().unwrap().clone()
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
    /// agents advertising the `pcap` capability. See
    /// [`crate::server::routing::resolve`] for the rules.
    pub(crate) fn resolve_source(
        &self,
        agents: &AgentRegistry,
        event: Option<&serde_json::Value>,
        explicit: Option<&str>,
    ) -> Result<ResolvedPcapSource, RouteError> {
        let routing = self.routing.read().unwrap();
        match routing::resolve(
            agents,
            CAPABILITY_PCAP,
            self.source.is_some(),
            &routing,
            event,
            explicit,
        )? {
            Resolved::Local => Ok(self.local_source().expect("local source is configured")),
            Resolved::Agent(entry) => Ok(ResolvedPcapSource::Agent(entry)),
        }
    }

    /// Test-only permit-release observability. The consuming suites
    /// exercise extraction, so Windows compiles them out.
    #[cfg(all(test, not(windows)))]
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
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Missing Npcap disables only the local source, not remote routing.
    let directory = directory.filter(|_| {
        if let Err(err) = crate::pcap::ensure_available() {
            warn!("Ignoring pcap.directory: {err}");
            false
        } else {
            true
        }
    });

    let spool = directory.map(|directory| {
        let directory = PathBuf::from(directory);
        if !directory.is_dir() {
            warn!(
                "PCAP spool directory {} does not exist (yet)",
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
    use crate::server::routing::sensor_identity;

    fn register_agent(registry: &AgentRegistry, name: &str, hostname: &str) -> Arc<AgentEntry> {
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        registry
            .register(
                name.to_string(),
                AgentHandshake {
                    hostname: hostname.to_string(),
                    version: "test".to_string(),
                    capabilities: vec![CAPABILITY_PCAP.to_string()],
                },
                None,
                "127.0.0.1:0".parse().unwrap(),
                tx,
            )
            .unwrap()
    }

    /// A Config backed only by a YAML file (no CLI arguments).
    fn yaml_config(dir: &std::path::Path, yaml: &str) -> crate::config::Config {
        let path = dir.join("evebox.yaml");
        std::fs::write(&path, yaml).unwrap();
        let args = clap::Command::new("test").get_matches_from(["test"]);
        crate::config::Config::new(args, path.to_str()).unwrap()
    }

    /// Unix always has a linked local backend; Windows needs Npcap.
    #[test]
    fn test_configure_local_spool() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = format!("pcap:\n  directory: {}\n", dir.path().display());
        let config = yaml_config(dir.path(), &yaml);
        let service = configure(&config);
        assert_eq!(
            service.has_source(),
            crate::pcap::ensure_available().is_ok()
        );
        assert_eq!(service.settings.max_bytes, 8_000_000);
        if !service.has_source() {
            // An unavailable local backend must not affect remote routing.
            let agents = AgentRegistry::default();
            register_agent(&agents, "remote", "remote-host");
            assert!(matches!(
                service
                    .resolve_source(&agents, None, Some("remote"))
                    .unwrap(),
                ResolvedPcapSource::Agent(_)
            ));
            return;
        }
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
    #[cfg(not(windows))]
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

    fn rule(sensor: &str, source: &str) -> PcapRoutingRule {
        PcapRoutingRule {
            sensor: sensor.to_string(),
            source: source.to_string(),
        }
    }

    fn routing(rules: Vec<PcapRoutingRule>, default: Option<&str>) -> PcapRouting {
        PcapRouting {
            rules,
            default: default.map(str::to_string),
        }
    }

    fn spooled_service(dir: &std::path::Path) -> PcapService {
        PcapService::new(
            PcapSettings::default(),
            Some(PcapSource::Spool(SpoolConfig::new(dir, None))),
        )
    }

    #[test]
    fn routing_rule_hit() {
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        register_agent(&agents, "fw-west", "host-b");
        service.set_routing(routing(vec![rule("sensor-1", "fw-west")], None));
        let event = serde_json::json!({ "host": "sensor-1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None).unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "fw-west"
        ));
    }

    #[test]
    fn routing_rule_order_first_wins() {
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        register_agent(&agents, "fw-west", "host-b");
        service.set_routing(routing(
            vec![rule("sensor-1", "fw-east"), rule("sensor-1", "fw-west")],
            None,
        ));
        let event = serde_json::json!({ "host": "sensor-1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None).unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "fw-east"
        ));
    }

    #[test]
    fn routing_rule_disconnected_source() {
        // A rule targeting a disconnected source fails with the SOURCE
        // name so the error can say which configured source is down.
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        service.set_routing(routing(vec![rule("sensor-1", "fw-agent")], None));
        let event = serde_json::json!({ "host": "sensor-1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None),
            Err(RouteError::NoSource(Some(name))) if name == "fw-agent"
        ));
    }

    #[test]
    fn routing_default() {
        // An identity with no matching rule, an event with no identity
        // at all, and a standalone request all route to the default.
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        register_agent(&agents, "fw-west", "host-b");
        service.set_routing(routing(vec![rule("sensor-1", "fw-west")], Some("fw-east")));
        for event in [
            Some(serde_json::json!({ "host": "unknown-sensor" })),
            Some(serde_json::json!({ "src_ip": "10.1.1.1" })),
            None,
        ] {
            assert!(matches!(
                service.resolve_source(&agents, event.as_ref(), None).unwrap(),
                ResolvedPcapSource::Agent(entry) if entry.name == "fw-east"
            ));
        }
    }

    #[test]
    fn routing_disconnected_default() {
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        service.set_routing(routing(vec![], Some("fw-agent")));
        let event = serde_json::json!({ "host": "unknown-sensor" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None),
            Err(RouteError::NoSource(Some(name))) if name == "fw-agent"
        ));
    }

    #[test]
    fn routing_no_match_no_default() {
        // With a table present routing is fully operator-controlled:
        // unmatched events must NOT fall through to the implicit
        // heuristics, even with a single connected source or a local
        // spool that would otherwise serve them.
        let dir = tempfile::tempdir().unwrap();
        let service = spooled_service(dir.path());
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        service.set_routing(routing(vec![rule("sensor-1", "fw-east")], None));

        // The no-rule error carries the sensor distinctly from
        // NoSource: connecting a source named "sensor-2" would not
        // help while the table is in force.
        let event = serde_json::json!({ "host": "sensor-2" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None),
            Err(RouteError::NoRule(Some(name))) if name == "sensor-2"
        ));

        // A no-identity event would implicitly ride the local spool,
        // and a standalone request a sole source; with a table and no
        // default they must not.
        let event = serde_json::json!({ "src_ip": "10.1.1.1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None),
            Err(RouteError::NoRule(None))
        ));
        assert!(matches!(
            service.resolve_source(&agents, None, None),
            Err(RouteError::NoRule(None))
        ));
    }

    #[test]
    fn routing_explicit_overrides_table() {
        let dir = tempfile::tempdir().unwrap();
        let service = spooled_service(dir.path());
        let agents = AgentRegistry::default();
        register_agent(&agents, "fw-east", "host-a");
        register_agent(&agents, "fw-west", "host-b");
        service.set_routing(routing(vec![rule("sensor-1", "fw-east")], None));
        let event = serde_json::json!({ "host": "sensor-1" });
        assert!(matches!(
            service
                .resolve_source(&agents, Some(&event), Some("fw-west"))
                .unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "fw-west"
        ));
        assert!(matches!(
            service
                .resolve_source(&agents, Some(&event), Some(LOCAL_PCAP_SOURCE_NAME))
                .unwrap(),
            ResolvedPcapSource::Local { name, .. } if name == LOCAL_PCAP_SOURCE_NAME
        ));
    }

    #[test]
    fn routing_overrides_agent_id_stamp() {
        // The canonical routing-table use case: a central agent
        // imports the events (and stamps them) while another source
        // holds the sensor's packets. The table must beat the stamp.
        let service = PcapService::default();
        let agents = AgentRegistry::default();
        register_agent(&agents, "importer", "host-a");
        register_agent(&agents, "fw-west", "host-b");
        service.set_routing(routing(vec![rule("sensor-1", "fw-west")], None));
        let event = serde_json::json!({
            "host": "sensor-1",
            "evebox": { "agent": { "id": "importer", "hostname": "host-a" } }
        });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None).unwrap(),
            ResolvedPcapSource::Agent(entry) if entry.name == "fw-west"
        ));
    }

    #[test]
    fn routing_local_spool_target() {
        // The reserved local-spool name works as a rule target, and a
        // rule naming it without a configured spool reports it down.
        let dir = tempfile::tempdir().unwrap();
        let service = spooled_service(dir.path());
        let agents = AgentRegistry::default();
        service.set_routing(routing(
            vec![rule("sensor-1", LOCAL_PCAP_SOURCE_NAME)],
            None,
        ));
        let event = serde_json::json!({ "host": "sensor-1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None).unwrap(),
            ResolvedPcapSource::Local { name, .. } if name == LOCAL_PCAP_SOURCE_NAME
        ));

        let unspooled = PcapService::default();
        unspooled.set_routing(routing(
            vec![rule("sensor-1", LOCAL_PCAP_SOURCE_NAME)],
            None,
        ));
        assert!(matches!(
            unspooled.resolve_source(&agents, Some(&event), None),
            Err(RouteError::NoSource(Some(name))) if name == LOCAL_PCAP_SOURCE_NAME
        ));
    }

    #[test]
    fn routing_empty_table_is_absent() {
        // An empty table is absent: the implicit heuristics apply.
        let dir = tempfile::tempdir().unwrap();
        let service = spooled_service(dir.path());
        let agents = AgentRegistry::default();
        service.set_routing(PcapRouting::default());
        let event = serde_json::json!({ "src_ip": "10.1.1.1" });
        assert!(matches!(
            service.resolve_source(&agents, Some(&event), None).unwrap(),
            ResolvedPcapSource::Local { name, .. } if name == LOCAL_PCAP_SOURCE_NAME
        ));
    }
}
