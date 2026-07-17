// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

#![cfg_attr(windows, allow(dead_code))]

//! Connected EveBox agents.
//!
//! This registry deliberately models the general agent control channel rather
//! than packet-capture sources. Packet capture is its first consumer, but
//! future capabilities can share the same connection and lifecycle.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

use crate::agent::protocol::{AgentHandshake, AgentMessage, CAPABILITY_PCAP, ServerMessage};
use crate::datetime::DateTime;
use crate::prelude::*;

/// Capacity of each connection's outbound control-message queue.
///
/// Dispatch uses `try_send`, so a slow or wedged peer cannot build an
/// unbounded server-side command queue.
pub(crate) const OUTBOUND_CAPACITY: usize = 32;

/// Reserved name of the server-local PCAP spool.
///
/// The parentheses keep it outside the space of plausible sensor names. An
/// agent claiming this name stays connected, but it is excluded from PCAP
/// routing so it can never shadow the server's own spool.
pub(crate) const LOCAL_PCAP_SOURCE_NAME: &str = "(server)";

/// Deliberately loose bound shared by agent keys, handshakes, and PCAP
/// routing. Agent names are normally host-name-sized; this primarily keeps
/// accidental or hostile input from growing without limit.
pub(crate) const MAX_AGENT_NAME_BYTES: usize = 16 * 1024;

/// Identity of one particular connection, as opposed to the agent's claimed
/// name which remains the same across reconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentConnectionId {
    pub(crate) name: String,
    pub(crate) generation: u64,
}

/// The agent key a connection authenticated with. `None` only under
/// `agents.allow-unauthenticated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentKeyIdentity {
    pub(crate) id: i64,
    pub(crate) name: String,
}

/// A registration refusal already logged by the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistrationRefused {
    NameHeldByOtherKey,
    RevokedKey,
}

/// Consumer of messages and lifecycle events received on the general agent
/// channel.
///
/// `pcap::tasks::Registry` implements this interface to accept terminal
/// results after validating the job token. Keeping the callback here avoids
/// coupling the general registry and WebSocket loop to a packet-capture job
/// implementation.
pub(crate) trait AgentMessageHandler: Send + Sync + 'static {
    /// Handle one decoded message.
    fn message(&self, connection: &AgentConnectionId, message: AgentMessage);

    /// Notify consumers that a particular connection ended. Consumers must
    /// use `generation` when failing pending work: a replacement connection
    /// with the same name may already be registered.
    fn disconnected(&self, _connection: &AgentConnectionId) {}
}

#[derive(Default)]
struct IgnoreAgentMessages;

impl AgentMessageHandler for IgnoreAgentMessages {
    fn message(&self, _connection: &AgentConnectionId, _message: AgentMessage) {}
}

/// A connected agent and its outbound control-channel handle.
pub(crate) struct AgentEntry {
    pub(crate) name: String,
    pub(crate) hostname: String,
    pub(crate) version: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) connected_at: DateTime,
    pub(crate) generation: u64,
    pub(crate) key: Option<AgentKeyIdentity>,
    pub(crate) remote: SocketAddr,
    pub(crate) outbound: mpsc::Sender<ServerMessage>,
    /// One server-side in-flight PCAP slot for this claimed source name.
    ///
    /// The registry reuses this semaphore across reconnects while an old job
    /// still holds it. That prevents a reconnect from creating a second lane
    /// around an extraction already in progress.
    pub(crate) pcap_busy: Arc<Semaphore>,
    last_seen: RwLock<DateTime>,
    /// `u64::MAX` means no ping round-trip has been observed yet.
    rtt_ms: AtomicU64,
    shutdown: CancellationToken,
}

impl AgentEntry {
    pub(crate) fn connection_id(&self) -> AgentConnectionId {
        AgentConnectionId {
            name: self.name.clone(),
            generation: self.generation,
        }
    }

    pub(crate) fn touch(&self) {
        *self.last_seen.write().unwrap() = DateTime::now();
    }

    pub(crate) fn set_rtt(&self, rtt_ms: u64) {
        self.rtt_ms.store(rtt_ms, Ordering::Relaxed);
    }

    pub(crate) fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|value| value == capability)
    }

    /// Queue a control message without waiting for capacity.
    #[allow(clippy::result_large_err)]
    pub(crate) fn try_send(
        &self,
        message: ServerMessage,
    ) -> Result<(), mpsc::error::TrySendError<ServerMessage>> {
        self.outbound.try_send(message)
    }

    pub(crate) async fn cancelled(&self) {
        self.shutdown.cancelled().await;
    }

    #[cfg(test)]
    fn is_cancelled(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

/// One row returned by `GET /api/agents`.
#[derive(Debug, Serialize)]
pub(crate) struct AgentInfo {
    pub(crate) name: String,
    pub(crate) hostname: String,
    pub(crate) version: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) connected_at: DateTime,
    pub(crate) last_seen: DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rtt_ms: Option<u64>,
}

#[derive(Default)]
struct RegistryState {
    agents: HashMap<String, Arc<AgentEntry>>,
    /// Key IDs deleted through the in-process admin API. Keeping them here
    /// closes the race where an upgrade authenticated just before deletion but
    /// reaches registration just after the live connection was bumped.
    revoked_key_ids: HashSet<i64>,
}

/// Live connected agents, keyed by their claimed name.
pub(crate) struct AgentRegistry {
    state: RwLock<RegistryState>,
    next_generation: AtomicU64,
    handler: Arc<dyn AgentMessageHandler>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new(Arc::new(IgnoreAgentMessages))
    }
}

impl AgentRegistry {
    pub(crate) fn new(handler: Arc<dyn AgentMessageHandler>) -> Self {
        Self {
            state: RwLock::new(RegistryState::default()),
            next_generation: AtomicU64::new(1),
            handler,
        }
    }

    /// Register a connection, replacing any older connection with the same
    /// claimed name and key identity. The old task is explicitly stopped;
    /// generation checks keep its eventual cleanup from removing the
    /// replacement.
    ///
    /// A held name is only replaceable by the same key identity (the
    /// half-open-TCP-after-restart case); a different key claiming it is
    /// refused so one credentialed sensor cannot evict another. Without
    /// identities to compare (`agents.allow-unauthenticated`) this falls
    /// back to replace-with-warn.
    pub(crate) fn register(
        &self,
        handshake: AgentHandshake,
        key: Option<AgentKeyIdentity>,
        remote: SocketAddr,
        outbound: mpsc::Sender<ServerMessage>,
    ) -> Result<Arc<AgentEntry>, RegistrationRefused> {
        let mut state = self.state.write().unwrap();

        if let Some(key) = &key
            && state.revoked_key_ids.contains(&key.id)
        {
            warn!(
                "REFUSING agent {:?} from {remote}: key {:?} was revoked",
                handshake.name, key.name
            );
            return Err(RegistrationRefused::RevokedKey);
        }

        if let Some(previous) = state.agents.get(&handshake.name)
            && let (Some(held), Some(claimed)) = (&previous.key, &key)
            && held.id != claimed.id
        {
            warn!(
                "REFUSING agent {:?} from {remote} using key {:?}: the name is held by a connection from {} authenticated with key {:?}",
                handshake.name, claimed.name, previous.remote, held.name
            );
            return Err(RegistrationRefused::NameHeldByOtherKey);
        }

        // Carry the PCAP slot over from any entry being replaced so a
        // reconnect cannot open a second lane around an extraction that is
        // still holding the old one.
        let pcap_busy = state
            .agents
            .get(&handshake.name)
            .map(|previous| previous.pcap_busy.clone())
            .unwrap_or_else(|| Arc::new(Semaphore::new(1)));

        let now = DateTime::now();
        let entry = Arc::new(AgentEntry {
            name: handshake.name.clone(),
            hostname: handshake.hostname,
            version: handshake.version,
            capabilities: handshake.capabilities,
            connected_at: now.clone(),
            generation: self.next_generation.fetch_add(1, Ordering::Relaxed),
            key,
            remote,
            outbound,
            pcap_busy,
            last_seen: RwLock::new(now),
            rtt_ms: AtomicU64::new(u64::MAX),
            shutdown: CancellationToken::new(),
        });

        if let Some(previous) = state.agents.insert(entry.name.clone(), entry.clone()) {
            warn!(
                "Agent {:?} replaced connection generation {} with generation {}",
                entry.name, previous.generation, entry.generation
            );
            previous.shutdown.cancel();
        }

        if entry.supports(CAPABILITY_PCAP) && entry.name == LOCAL_PCAP_SOURCE_NAME {
            warn!(
                "Agent {:?} uses the reserved server-local PCAP source name; keeping the control channel but disabling its remote PCAP capability",
                entry.name
            );
        }

        Ok(entry)
    }

    /// Remove this generation only. Returns false when a replacement already
    /// owns the name.
    pub(crate) fn remove(&self, connection: &AgentConnectionId) -> bool {
        let mut state = self.state.write().unwrap();
        let is_current = state
            .agents
            .get(&connection.name)
            .is_some_and(|entry| entry.generation == connection.generation);
        if is_current {
            state.agents.remove(&connection.name);
        }
        is_current
    }

    pub(crate) fn get(&self, name: &str) -> Option<Arc<AgentEntry>> {
        self.state.read().unwrap().agents.get(name).cloned()
    }

    /// Revoke `key_id` in the running registry and ask its connection, if any,
    /// to disconnect. The in-memory revocation closes the narrow race with an
    /// upgrade that authenticated immediately before the database deletion.
    pub(crate) fn revoke_key(&self, key_id: i64) -> bool {
        let mut state = self.state.write().unwrap();
        state.revoked_key_ids.insert(key_id);
        let entry = state
            .agents
            .values()
            .find(|entry| entry.key.as_ref().is_some_and(|key| key.id == key_id))
            .cloned();
        drop(state);
        if let Some(entry) = entry {
            entry.shutdown.cancel();
            true
        } else {
            false
        }
    }

    /// Dispatch only while this exact generation is still the registry's
    /// current connection. Holding the registry read lock closes the race in
    /// which teardown notifies pending jobs and a stale-but-open MPSC sender
    /// then accepts a new request.
    #[allow(clippy::result_large_err)]
    pub(crate) fn try_send_current(
        &self,
        entry: &AgentEntry,
        message: ServerMessage,
    ) -> Result<(), mpsc::error::TrySendError<ServerMessage>> {
        let state = self.state.read().unwrap();
        if !state
            .agents
            .get(&entry.name)
            .is_some_and(|current| current.generation == entry.generation)
        {
            return Err(mpsc::error::TrySendError::Closed(message));
        }
        entry.try_send(message)
    }

    /// Return one agent only when its advertised PCAP capability is eligible
    /// for routing (including the reserved-name exclusion).
    pub(crate) fn pcap_agent(&self, name: &str) -> Option<Arc<AgentEntry>> {
        if name == LOCAL_PCAP_SOURCE_NAME {
            return None;
        }
        self.get(name)
            .filter(|entry| entry.supports(CAPABILITY_PCAP))
    }

    /// Connected, eligible PCAP agents sorted by name.
    pub(crate) fn pcap_agents(&self) -> Vec<Arc<AgentEntry>> {
        let mut entries: Vec<Arc<AgentEntry>> = self
            .state
            .read()
            .unwrap()
            .agents
            .values()
            .filter(|entry| entry.supports(CAPABILITY_PCAP) && entry.name != LOCAL_PCAP_SOURCE_NAME)
            .cloned()
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    pub(crate) fn has_pcap(&self) -> bool {
        self.state
            .read()
            .unwrap()
            .agents
            .values()
            .any(|entry| entry.supports(CAPABILITY_PCAP) && entry.name != LOCAL_PCAP_SOURCE_NAME)
    }

    pub(crate) fn connected(&self) -> usize {
        self.state.read().unwrap().agents.len()
    }

    /// General connected-agent read model, sorted by claimed name.
    pub(crate) fn list(&self) -> Vec<AgentInfo> {
        let mut rows: Vec<AgentInfo> = self
            .state
            .read()
            .unwrap()
            .agents
            .values()
            .map(|entry| {
                let rtt_ms = entry.rtt_ms.load(Ordering::Relaxed);
                AgentInfo {
                    name: entry.name.clone(),
                    hostname: entry.hostname.clone(),
                    version: entry.version.clone(),
                    capabilities: entry.capabilities.clone(),
                    connected_at: entry.connected_at.clone(),
                    last_seen: entry.last_seen.read().unwrap().clone(),
                    rtt_ms: (rtt_ms != u64::MAX).then_some(rtt_ms),
                }
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    pub(crate) fn handle_message(&self, connection: &AgentConnectionId, message: AgentMessage) {
        self.handler.message(connection, message);
    }

    pub(crate) fn disconnected(&self, connection: &AgentConnectionId) {
        self.handler.disconnected(connection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handshake(name: &str, capabilities: &[&str]) -> AgentHandshake {
        AgentHandshake {
            name: name.to_string(),
            hostname: format!("{name}.example.test"),
            version: "0.27.0-dev".to_string(),
            capabilities: capabilities.iter().map(|value| value.to_string()).collect(),
        }
    }

    fn remote() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    fn key(id: i64) -> Option<AgentKeyIdentity> {
        Some(AgentKeyIdentity {
            id,
            name: format!("key-{id}"),
        })
    }

    fn register(registry: &AgentRegistry, name: &str, capabilities: &[&str]) -> Arc<AgentEntry> {
        let (tx, _rx) = mpsc::channel(OUTBOUND_CAPACITY);
        registry
            .register(handshake(name, capabilities), None, remote(), tx)
            .unwrap()
    }

    fn register_with_key(
        registry: &AgentRegistry,
        name: &str,
        key: Option<AgentKeyIdentity>,
    ) -> Result<Arc<AgentEntry>, RegistrationRefused> {
        let (tx, _rx) = mpsc::channel(OUTBOUND_CAPACITY);
        registry.register(handshake(name, &[CAPABILITY_PCAP]), key, remote(), tx)
    }

    #[test]
    fn replacement_and_removal_are_generation_safe() {
        let registry = AgentRegistry::default();
        let first = register(&registry, "sensor-a", &[CAPABILITY_PCAP]);
        let second = register(&registry, "sensor-a", &[CAPABILITY_PCAP]);

        assert_ne!(first.generation, second.generation);
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        assert_eq!(registry.connected(), 1);

        assert!(!registry.remove(&first.connection_id()));
        assert_eq!(
            registry.get("sensor-a").unwrap().generation,
            second.generation
        );
        assert!(registry.remove(&second.connection_id()));
        assert_eq!(registry.connected(), 0);
    }

    #[test]
    fn replacement_keeps_the_held_pcap_slot() {
        let registry = AgentRegistry::default();
        let first = register(&registry, "sensor-a", &[CAPABILITY_PCAP]);
        let permit = first.pcap_busy.clone().try_acquire_owned().unwrap();

        let second = register(&registry, "sensor-a", &[CAPABILITY_PCAP]);
        assert!(second.pcap_busy.clone().try_acquire_owned().is_err());

        drop(permit);
        assert!(second.pcap_busy.clone().try_acquire_owned().is_ok());
    }

    #[test]
    fn reserved_local_name_excludes_only_the_pcap_projection() {
        let registry = AgentRegistry::default();
        let entry = register(
            &registry,
            LOCAL_PCAP_SOURCE_NAME,
            &[CAPABILITY_PCAP, "future-rules"],
        );

        assert!(registry.get(LOCAL_PCAP_SOURCE_NAME).is_some());
        assert!(entry.supports("future-rules"));
        assert!(registry.pcap_agent(LOCAL_PCAP_SOURCE_NAME).is_none());
        assert!(registry.pcap_agents().is_empty());
        assert!(!registry.has_pcap());
        assert_eq!(registry.list()[0].capabilities.len(), 2);
    }

    #[test]
    fn pcap_projection_and_agent_list_are_sorted() {
        let registry = AgentRegistry::default();
        register(&registry, "zeta", &[CAPABILITY_PCAP]);
        let alpha = register(&registry, "alpha", &[CAPABILITY_PCAP]);
        alpha.set_rtt(17);

        let pcap_names: Vec<String> = registry
            .pcap_agents()
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        assert_eq!(pcap_names, ["alpha", "zeta"]);

        let rows = registry.list();
        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[0].rtt_ms, Some(17));
        assert_eq!(rows[1].name, "zeta");
        assert_eq!(rows[1].rtt_ms, None);

        let json = serde_json::to_value(&rows[1]).unwrap();
        assert_eq!(json["capabilities"], serde_json::json!(["pcap"]));
        assert!(json.get("connected_at").is_some());
        assert!(json.get("last_seen").is_some());
        assert!(json.get("rtt_ms").is_none());
    }

    #[test]
    fn outbound_queue_is_bounded_and_nonblocking() {
        let registry = AgentRegistry::default();
        let (tx, _rx) = mpsc::channel(1);
        let entry = registry
            .register(handshake("sensor-a", &[]), None, remote(), tx)
            .unwrap();

        entry.try_send(ServerMessage::Unknown).unwrap();
        assert!(matches!(
            entry.try_send(ServerMessage::Unknown),
            Err(mpsc::error::TrySendError::Full(_))
        ));
    }

    #[test]
    fn same_key_replaces_but_a_different_key_is_refused() {
        let registry = AgentRegistry::default();
        let first = register_with_key(&registry, "sensor-a", key(1)).unwrap();

        // A different key cannot evict the held name.
        assert_eq!(
            register_with_key(&registry, "sensor-a", key(2)).err(),
            Some(RegistrationRefused::NameHeldByOtherKey)
        );
        assert!(!first.is_cancelled());
        assert_eq!(
            registry.get("sensor-a").unwrap().generation,
            first.generation
        );

        // The same key identity replaces its own stale connection.
        let second = register_with_key(&registry, "sensor-a", key(1)).unwrap();
        assert!(first.is_cancelled());
        assert_eq!(
            registry.get("sensor-a").unwrap().generation,
            second.generation
        );

        // A different key is free to claim a different name.
        register_with_key(&registry, "sensor-b", key(2)).unwrap();
        assert_eq!(registry.connected(), 2);
    }

    #[test]
    fn revoking_a_key_cancels_its_connection_and_refuses_racing_registration() {
        let registry = AgentRegistry::default();
        let first = register_with_key(&registry, "sensor-a", key(1)).unwrap();
        let second = register_with_key(&registry, "sensor-b", key(2)).unwrap();
        let unauthenticated = register_with_key(&registry, "sensor-c", None).unwrap();

        assert!(registry.revoke_key(1));
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        assert!(!unauthenticated.is_cancelled());
        assert_eq!(
            register_with_key(&registry, "sensor-a", key(1)).err(),
            Some(RegistrationRefused::RevokedKey)
        );
        assert!(!registry.revoke_key(99));
    }

    #[test]
    fn without_identities_replacement_falls_back_to_last_writer() {
        let registry = AgentRegistry::default();

        // Both unauthenticated: no identities to compare.
        let first = register_with_key(&registry, "sensor-a", None).unwrap();
        let second = register_with_key(&registry, "sensor-a", None).unwrap();
        assert!(first.is_cancelled());

        // A keyed claim over an unauthenticated holder (or the reverse)
        // also has nothing to compare and replaces.
        let third = register_with_key(&registry, "sensor-a", key(1)).unwrap();
        assert!(second.is_cancelled());
        let fourth = register_with_key(&registry, "sensor-a", None).unwrap();
        assert!(third.is_cancelled());
        assert!(!fourth.is_cancelled());
    }

    #[test]
    fn stale_generation_cannot_accept_new_requests() {
        let registry = AgentRegistry::default();
        let (first_tx, mut first_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let first = registry
            .register(
                handshake("sensor-a", &[CAPABILITY_PCAP]),
                None,
                remote(),
                first_tx,
            )
            .unwrap();
        let (second_tx, _second_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        registry
            .register(
                handshake("sensor-a", &[CAPABILITY_PCAP]),
                None,
                remote(),
                second_tx,
            )
            .unwrap();

        assert!(matches!(
            registry.try_send_current(&first, ServerMessage::Unknown),
            Err(mpsc::error::TrySendError::Closed(ServerMessage::Unknown))
        ));
        assert!(first_rx.try_recv().is_err());
    }
}
