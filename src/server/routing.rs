// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Event to data-source routing shared by the sensor-side retrieval
//! features (packet capture and extracted files).
//!
//! A source is either the server-local store, always named
//! [`LOCAL_PCAP_SOURCE_NAME`], or a connected agent advertising the
//! requested capability. The operator routing table (persisted as the
//! PCAP routing table) maps sensors to sources for every capability.

use crate::server::agents::{AgentEntry, AgentRegistry, LOCAL_PCAP_SOURCE_NAME};
use crate::server::pcap::PcapRouting;
use std::sync::Arc;

/// A source selected by [`resolve`], independent of what it serves.
pub(crate) enum Resolved {
    /// The server-local store.
    Local,
    /// A connected agent advertising the requested capability.
    Agent(Arc<AgentEntry>),
}

impl Resolved {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Local => LOCAL_PCAP_SOURCE_NAME,
            Self::Agent(entry) => &entry.name,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RouteError {
    NoSource(Option<String>),
    Ambiguous(Vec<String>),
    /// The operator routing table is in force but no rule matched the
    /// event's sensor identity (carried when it had one) and no
    /// default source is set. Distinct from `NoSource`: the remedy is
    /// a rule or default, not connecting a source by this name.
    NoRule(Option<String>),
}

/// The source a name selects right now: the reserved `(server)` name is
/// the local store, anything else a live agent advertising `capability`.
fn source_by_name(
    agents: &AgentRegistry,
    capability: &str,
    has_local: bool,
    name: &str,
) -> Option<Resolved> {
    if name == LOCAL_PCAP_SOURCE_NAME {
        has_local.then_some(Resolved::Local)
    } else {
        agents.capable_agent(name, capability).map(Resolved::Agent)
    }
}

fn ambiguous(sources: &[Resolved]) -> RouteError {
    let mut names: Vec<String> = sources
        .iter()
        .map(|source| source.name().to_string())
        .collect();
    names.sort();
    RouteError::Ambiguous(names)
}

/// Resolve a request across the optional server-local store and live
/// agents advertising `capability`.
///
/// An explicit source name always wins. Otherwise, when the operator
/// routing table is present it is fully in control: the first rule
/// whose sensor equals the event's identity, else the default
/// source, else no source. Without a table the implicit heuristics
/// apply:
///
/// Agents are matched by the event's sensor identity, then its EveBox
/// agent identifier stamp, then the older hostname stamp. The local
/// store has no identity to match: it serves explicit `(server)`
/// requests and events with no agent stamp — those were ingested by
/// this server's own input, so the local Suricata output holds their
/// data whatever their sensor identity says. Stamped events whose agent
/// is gone are never quietly served from the local store.
pub(crate) fn resolve(
    agents: &AgentRegistry,
    capability: &str,
    has_local: bool,
    routing: &PcapRouting,
    event: Option<&serde_json::Value>,
    explicit: Option<&str>,
) -> Result<Resolved, RouteError> {
    if let Some(name) = explicit {
        return source_by_name(agents, capability, has_local, name)
            .ok_or_else(|| RouteError::NoSource(Some(name.to_string())));
    }

    let identity = event.and_then(sensor_identity);

    if !routing.is_empty() {
        let target = routing
            .rules
            .iter()
            .find(|rule| identity == Some(rule.sensor.as_str()))
            .map(|rule| rule.source.as_str())
            .or(routing.default.as_deref());
        return match target {
            // A disconnected target carries the SOURCE name so the error
            // can say which configured source is down, not just which
            // sensor went unmatched.
            Some(name) => source_by_name(agents, capability, has_local, name)
                .ok_or_else(|| RouteError::NoSource(Some(name.to_string()))),
            None => Err(RouteError::NoRule(identity.map(str::to_string))),
        };
    }

    if let Some(name) = identity
        && let Some(entry) = agents.capable_agent(name, capability)
    {
        return Ok(Resolved::Agent(entry));
    }

    // The importer stamp is exact: an event carrying `evebox.agent.id`
    // was imported by that agent, so only that agent can serve the event.
    // The fuzzier hostname stamp remains for events imported before the
    // identifier stamp existed.
    if let Some(id) = event.and_then(stamped_agent_id) {
        return agents
            .capable_agent(id, capability)
            .map(Resolved::Agent)
            .ok_or_else(|| RouteError::NoSource(Some(id.to_string())));
    }

    if let Some(hostname) = event.and_then(agent_hostname) {
        let mut matches: Vec<Resolved> = agents
            .capable_agents(capability)
            .into_iter()
            .filter(|entry| entry.hostname == hostname)
            .map(Resolved::Agent)
            .collect();
        return match matches.len() {
            0 => Err(RouteError::NoSource(
                identity.or(Some(hostname)).map(ToOwned::to_owned),
            )),
            1 => Ok(matches.pop().expect("one source")),
            _ => Err(ambiguous(&matches)),
        };
    }

    // An unstamped event came in through this server's own input.
    if event.is_some() {
        if has_local {
            return Ok(Resolved::Local);
        }
        if let Some(name) = identity {
            return Err(RouteError::NoSource(Some(name.to_string())));
        }
    }

    // A standalone request, or an anonymous event with no local store: a
    // single available source serves it; more than one must be chosen
    // explicitly.
    let mut sources = Vec::new();
    if has_local {
        sources.push(Resolved::Local);
    }
    sources.extend(
        agents
            .capable_agents(capability)
            .into_iter()
            .map(Resolved::Agent),
    );
    match sources.len() {
        0 => Err(RouteError::NoSource(None)),
        1 => Ok(sources.pop().expect("one source")),
        _ => Err(ambiguous(&sources)),
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::agent::protocol::{AgentHandshake, CAPABILITY_FILESTORE, CAPABILITY_PCAP};
    use crate::server::pcap::PcapRoutingRule;

    fn register(registry: &AgentRegistry, name: &str, capabilities: &[&str]) {
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        registry
            .register(
                name.to_string(),
                AgentHandshake {
                    hostname: format!("{name}.host"),
                    version: "test".to_string(),
                    capabilities: capabilities.iter().map(|c| c.to_string()).collect(),
                },
                None,
                "127.0.0.1:0".parse().unwrap(),
                tx,
            )
            .unwrap();
    }

    fn names(result: Result<Resolved, RouteError>) -> Result<String, RouteError> {
        result.map(|resolved| resolved.name().to_string())
    }

    #[test]
    fn resolution_only_considers_agents_with_the_capability() {
        let registry = AgentRegistry::default();
        register(&registry, "pcap-only", &[CAPABILITY_PCAP]);
        register(&registry, "both", &[CAPABILITY_PCAP, CAPABILITY_FILESTORE]);
        let routing = PcapRouting::default();
        let event = serde_json::json!({ "host": "pcap-only" });

        // Sensor identity matches an agent without the capability, and the
        // event is unstamped: the local store serves it.
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_FILESTORE,
                true,
                &routing,
                Some(&event),
                None
            ))
            .unwrap(),
            LOCAL_PCAP_SOURCE_NAME
        );
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_PCAP,
                true,
                &routing,
                Some(&event),
                None
            ))
            .unwrap(),
            "pcap-only"
        );
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_FILESTORE,
                false,
                &routing,
                None,
                Some("pcap-only")
            )),
            Err(RouteError::NoSource(Some("pcap-only".to_string())))
        );
        // Standalone: one capable agent and no local store.
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_FILESTORE,
                false,
                &routing,
                None,
                None
            ))
            .unwrap(),
            "both"
        );
    }

    #[test]
    fn stamped_events_never_fall_back_to_the_local_store() {
        let registry = AgentRegistry::default();
        let event = serde_json::json!({ "host": "s1", "evebox": { "agent": { "id": "gone" } } });
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_FILESTORE,
                true,
                &PcapRouting::default(),
                Some(&event),
                None
            )),
            Err(RouteError::NoSource(Some("gone".to_string())))
        );
    }

    #[test]
    fn routing_table_applies_to_every_capability() {
        let registry = AgentRegistry::default();
        register(&registry, "collector", &[CAPABILITY_FILESTORE]);
        let routing = PcapRouting {
            rules: vec![PcapRoutingRule {
                sensor: "s1".to_string(),
                source: "collector".to_string(),
            }],
            default: None,
        };
        let event = serde_json::json!({ "host": "s1" });
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_FILESTORE,
                true,
                &routing,
                Some(&event),
                None
            ))
            .unwrap(),
            "collector"
        );
        assert_eq!(
            names(resolve(
                &registry,
                CAPABILITY_PCAP,
                true,
                &routing,
                Some(&event),
                None
            )),
            Err(RouteError::NoSource(Some("collector".to_string())))
        );
    }
}
