// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Shared protocol for the persistent EveBox agent control channel.
//!
//! The WebSocket carries small JSON control messages. Bulk data, including
//! packet captures, uses separate HTTP requests. Both directions deliberately
//! tolerate unknown message types and unknown fields so additive protocol
//! changes do not disconnect older peers.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// WebSocket subprotocol selected during the one-time HTTP upgrade.
pub(crate) const SUBPROTOCOL: &str = "evebox-agent-v1";

/// Upgrade-request header containing [`AgentHandshake`] as JSON.
pub(crate) const AGENT_HEADER: &str = "x-evebox-agent";

/// Path of the agent control-channel WebSocket endpoint.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) const AGENT_WS_PATH: &str = "/api/agent/ws";

/// Packet-capture control-channel capability.
pub(crate) const CAPABILITY_PCAP: &str = "pcap";

/// Maximum inbound control message or frame size on either peer.
pub(crate) const CONTROL_MESSAGE_MAX_BYTES: usize = 256 * 1024;

/// Content type the agent sends on the PCAP upload data plane and the
/// server's upload endpoint requires.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) const PCAP_CONTENT_TYPE: &str = "application/vnd.tcpdump.pcap";

/// Path of the per-job PCAP upload endpoint, as an axum route template.
/// [`agent_pcap_upload_path`] produces the matching concrete request path.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) const AGENT_PCAP_UPLOAD_ROUTE: &str = "/api/agent/pcap/{id}";

/// The concrete upload request path for one job.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn agent_pcap_upload_path(id: &str) -> String {
    format!("/api/agent/pcap/{id}")
}

/// Metadata asserted by an agent during the WebSocket upgrade.
///
/// Authentication and identity binding are intentionally deferred. Unknown
/// fields are ignored for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AgentHandshake {
    pub(crate) name: String,
    pub(crate) hostname: String,
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
}

/// One endpoint of a direction-symmetric flow selector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WireEndpoint {
    pub(crate) ip: IpAddr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) port: Option<u16>,
}

/// A normalized packet filter sent to an agent.
///
/// This wire DTO cannot carry a spool path. The agent always applies it to
/// its configured spool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum WirePcapFilter {
    Flow {
        proto: u8,
        a: WireEndpoint,
        b: WireEndpoint,
    },
    Expression {
        expression: String,
    },
    All,
}

/// Effective extraction limits selected by the server.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WireLimits {
    /// Maximum output bytes, including the classic pcap file header.
    pub(crate) max_bytes: u64,
    /// Maximum scan time; output backpressure is not charged to this limit.
    pub(crate) scan_timeout_ms: u64,
}

/// Terminal extraction statistics reported by an agent.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WireStats {
    pub(crate) packets: u64,
    /// Output bytes, including the classic pcap file header.
    pub(crate) bytes: u64,
    pub(crate) files_scanned: u32,
    pub(crate) files_vanished: u32,
    #[serde(default)]
    pub(crate) truncated: bool,
}

/// Terminal outcome of an accepted packet-capture job.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PcapResultCode {
    Complete,
    NoCandidateFiles,
    NoMatch,
    Cancelled,
    Error,
}

/// Whether a job used and completed its separate HTTP upload.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PcapUploadStatus {
    None,
    Complete,
    Failed,
}

/// Terminal outcome payload of an accepted packet-capture job, carried by
/// [`AgentMessage::PcapResult`] and shared by both peers' job bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PcapResult {
    pub(crate) code: PcapResultCode,
    pub(crate) upload: PcapUploadStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stats: Option<WireStats>,
}

#[cfg_attr(windows, allow(dead_code))]
impl PcapResult {
    pub(crate) fn error(message: String) -> Self {
        Self {
            code: PcapResultCode::Error,
            upload: PcapUploadStatus::None,
            message: Some(message),
            stats: None,
        }
    }

    pub(crate) fn cancelled(upload: PcapUploadStatus, stats: Option<WireStats>) -> Self {
        Self {
            code: PcapResultCode::Cancelled,
            upload,
            message: None,
            stats,
        }
    }
}

/// Messages sent from the server to an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum ServerMessage {
    /// The first server frame after the upgrade.
    Hello {
        server_version: String,
        #[serde(default)]
        capabilities: Vec<String>,
    },
    /// Execute a normalized request and upload any resulting bytes over HTTP.
    PcapRequest {
        id: String,
        token: String,
        filter: WirePcapFilter,
        /// Inclusive lower packet timestamp bound, as Unix microseconds.
        start_us: u64,
        /// Inclusive upper packet timestamp bound, as Unix microseconds.
        end_us: u64,
        limits: WireLimits,
    },
    /// Cancel a job. Best effort: a job whose control channel is gone is
    /// cancelled by the agent itself.
    Cancel { id: String, token: String },
    /// A message type this build does not understand.
    #[serde(other)]
    Unknown,
}

/// Messages sent from an agent to the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum AgentMessage {
    /// Exactly one terminal result is produced for every accepted job.
    PcapResult {
        id: String,
        token: String,
        #[serde(flatten)]
        result: PcapResult,
    },
    /// A message type this build does not understand.
    #[serde(other)]
    Unknown,
}

mod pcap_conversions {
    use std::time::Duration;

    use super::*;
    use crate::pcap::{FetchStats, FlowSelector, Limits, PcapFilter};

    impl From<&FlowSelector> for WirePcapFilter {
        fn from(selector: &FlowSelector) -> Self {
            Self::Flow {
                proto: selector.proto,
                a: WireEndpoint {
                    ip: selector.a.0,
                    port: selector.a.1,
                },
                b: WireEndpoint {
                    ip: selector.b.0,
                    port: selector.b.1,
                },
            }
        }
    }

    impl From<&PcapFilter> for WirePcapFilter {
        fn from(filter: &PcapFilter) -> Self {
            match filter {
                PcapFilter::Flow(selector) => selector.into(),
                PcapFilter::Expression(expression) => Self::Expression {
                    expression: expression.clone(),
                },
            }
        }
    }

    impl From<Option<&PcapFilter>> for WirePcapFilter {
        fn from(filter: Option<&PcapFilter>) -> Self {
            match filter {
                Some(filter) => filter.into(),
                None => Self::All,
            }
        }
    }

    impl WirePcapFilter {
        /// Convert the wire DTO to the engine filter. `All` maps to no
        /// filter. Agent-side only, so compiled out with extraction on
        /// Windows.
        #[cfg(not(windows))]
        pub(crate) fn into_pcap_filter(self) -> Option<PcapFilter> {
            match self {
                Self::Flow { proto, a, b } => Some(PcapFilter::Flow(FlowSelector {
                    proto,
                    a: (a.ip, a.port),
                    b: (b.ip, b.port),
                })),
                Self::Expression { expression } => Some(PcapFilter::Expression(expression)),
                Self::All => None,
            }
        }
    }

    impl From<WireLimits> for Limits {
        fn from(limits: WireLimits) -> Self {
            Self {
                max_bytes: limits.max_bytes,
                deadline: Some(Duration::from_millis(limits.scan_timeout_ms)),
                ..Self::default()
            }
        }
    }

    impl TryFrom<&Limits> for WireLimits {
        type Error = &'static str;

        fn try_from(limits: &Limits) -> Result<Self, Self::Error> {
            let deadline = limits.deadline.ok_or("scan timeout is required")?;
            let scan_timeout_ms = deadline
                .as_millis()
                .try_into()
                .map_err(|_| "scan timeout does not fit in u64 milliseconds")?;
            Ok(Self {
                max_bytes: limits.max_bytes,
                scan_timeout_ms,
            })
        }
    }

    impl From<&FetchStats> for WireStats {
        fn from(stats: &FetchStats) -> Self {
            Self {
                packets: stats.packets,
                bytes: stats.bytes,
                files_scanned: stats.files_scanned,
                files_vanished: stats.files_vanished,
                truncated: stats.truncated,
            }
        }
    }

    impl From<WireStats> for FetchStats {
        fn from(stats: WireStats) -> Self {
            Self {
                packets: stats.packets,
                bytes: stats.bytes,
                files_scanned: stats.files_scanned,
                files_vanished: stats.files_vanished,
                truncated: stats.truncated,
                linktype: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_defaults_capabilities_and_ignores_unknown_fields() {
        let handshake: AgentHandshake = serde_json::from_str(
            r#"{"name":"sensor","hostname":"host","version":"0.27","future":true}"#,
        )
        .unwrap();
        assert!(handshake.capabilities.is_empty());
        assert_eq!(handshake.name, "sensor");
    }

    #[test]
    fn hello_round_trips_and_ignores_unknown_fields() {
        let text =
            r#"{"type":"hello","server_version":"0.27.0","capabilities":["pcap"],"future":1}"#;
        let message: ServerMessage = serde_json::from_str(text).unwrap();
        assert_eq!(
            message,
            ServerMessage::Hello {
                server_version: "0.27.0".to_string(),
                capabilities: vec!["pcap".to_string()],
            }
        );
        let encoded = serde_json::to_string(&message).unwrap();
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&encoded).unwrap(),
            message
        );
        assert_eq!(
            serde_json::from_str::<ServerMessage>(r#"{"type":"hello","server_version":"0.27.0"}"#)
                .unwrap(),
            ServerMessage::Hello {
                server_version: "0.27.0".to_string(),
                capabilities: Vec::new(),
            }
        );
    }

    #[test]
    fn all_filter_request_has_normalized_wire_shape() {
        let message = ServerMessage::PcapRequest {
            id: "job-1".to_string(),
            token: "token-1".to_string(),
            filter: WirePcapFilter::All,
            start_us: 1_783_072_800_000_000,
            end_us: 1_783_073_111_000_000,
            limits: WireLimits {
                max_bytes: 8_000_000,
                scan_timeout_ms: 60_000,
            },
        };
        let text = serde_json::to_string(&message).unwrap();
        assert_eq!(
            text,
            r#"{"type":"pcap-request","id":"job-1","token":"token-1","filter":{"type":"all"},"start_us":1783072800000000,"end_us":1783073111000000,"limits":{"max_bytes":8000000,"scan_timeout_ms":60000}}"#
        );
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            message
        );
    }

    #[test]
    fn flow_filter_omits_absent_ports() {
        let filter = WirePcapFilter::Flow {
            proto: 1,
            a: WireEndpoint {
                ip: "192.0.2.1".parse().unwrap(),
                port: None,
            },
            b: WireEndpoint {
                ip: "192.0.2.2".parse().unwrap(),
                port: None,
            },
        };
        let text = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            text,
            r#"{"type":"flow","proto":1,"a":{"ip":"192.0.2.1"},"b":{"ip":"192.0.2.2"}}"#
        );
    }

    #[test]
    fn expression_filter_round_trips() {
        let filter = WirePcapFilter::Expression {
            expression: "tcp port 443".to_string(),
        };
        let text = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            serde_json::from_str::<WirePcapFilter>(&text).unwrap(),
            filter
        );
    }

    #[test]
    fn cancel_repeats_the_token() {
        let message = ServerMessage::Cancel {
            id: "job-2".to_string(),
            token: "token-2".to_string(),
        };
        let text = serde_json::to_string(&message).unwrap();
        assert!(text.contains(r#""token":"token-2""#));
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            message
        );
    }

    #[test]
    fn terminal_result_round_trips() {
        let message = AgentMessage::PcapResult {
            id: "job-3".to_string(),
            token: "token-3".to_string(),
            result: PcapResult {
                code: PcapResultCode::Complete,
                upload: PcapUploadStatus::Complete,
                message: None,
                stats: Some(WireStats {
                    packets: 42,
                    bytes: 8_192,
                    files_scanned: 4,
                    files_vanished: 1,
                    truncated: false,
                }),
            },
        };
        let text = serde_json::to_string(&message).unwrap();
        assert!(text.contains(r#""code":"complete""#));
        assert!(text.contains(r#""upload":"complete""#));
        assert_eq!(
            serde_json::from_str::<AgentMessage>(&text).unwrap(),
            message
        );
    }

    #[test]
    fn unknown_message_types_are_tolerated_in_both_directions() {
        assert_eq!(
            serde_json::from_str::<ServerMessage>(r#"{"type":"rules-updated","revision":42}"#)
                .unwrap(),
            ServerMessage::Unknown
        );
        assert_eq!(
            serde_json::from_str::<AgentMessage>(r#"{"type":"health-report","healthy":true}"#)
                .unwrap(),
            AgentMessage::Unknown
        );
    }

    // Exercises the agent-side wire-to-engine direction, which
    // Windows builds omit.
    #[cfg(not(windows))]
    #[test]
    fn pcap_filter_and_limits_convert_without_a_spool_path() {
        use std::time::Duration;

        use crate::pcap::{FlowSelector, Limits, PcapFilter};

        let selector = FlowSelector {
            proto: 6,
            a: ("192.0.2.1".parse().unwrap(), Some(42_000)),
            b: ("198.51.100.2".parse().unwrap(), Some(443)),
        };
        let engine = PcapFilter::Flow(selector);
        let wire = WirePcapFilter::from(Some(&engine));
        match wire.clone().into_pcap_filter().unwrap() {
            PcapFilter::Flow(round_trip) => {
                assert_eq!(round_trip.proto, 6);
                assert_eq!(round_trip.a.1, Some(42_000));
                assert_eq!(round_trip.b.1, Some(443));
            }
            _ => panic!("expected flow filter"),
        }
        let expression = PcapFilter::Expression("tcp port 443".to_string());
        assert_eq!(
            WirePcapFilter::from(&expression),
            WirePcapFilter::Expression {
                expression: "tcp port 443".to_string()
            }
        );
        assert_eq!(WirePcapFilter::from(None), WirePcapFilter::All);
        assert!(WirePcapFilter::All.into_pcap_filter().is_none());

        let limits = Limits {
            max_bytes: 8_000_000,
            deadline: Some(Duration::from_secs(60)),
            ..Limits::default()
        };
        let wire_limits = WireLimits::try_from(&limits).unwrap();
        assert_eq!(wire_limits.scan_timeout_ms, 60_000);
        let round_trip = Limits::from(wire_limits);
        assert_eq!(round_trip.max_bytes, limits.max_bytes);
        assert_eq!(round_trip.deadline, limits.deadline);
    }
}
