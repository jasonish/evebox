// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! EVE event to extraction window and flow selector derivation, used
//! by the server API to turn a stored event into a [`FlowSelector`]
//! and time window for [`crate::pcap::fetch`].

use std::net::IpAddr;

use anyhow::{anyhow, bail};

use crate::datetime::{self, DateTime};
use crate::prelude::*;

use super::filter::FlowSelector;

#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) start: DateTime,
    pub(crate) end: DateTime,
}

/// Parse an EVE timestamp, including the old offset format
/// (`-0600`).
fn parse_ts(input: &str) -> Result<DateTime> {
    datetime::parse(input, None).map_err(|err| anyhow!("bad timestamp {input:?}: {err}"))
}

fn event_timestamp(event: &serde_json::Value) -> Result<DateTime> {
    let timestamp = event["timestamp"]
        .as_str()
        .ok_or_else(|| anyhow!("event has no timestamp"))?;
    parse_ts(timestamp)
}

/// Checked timestamp arithmetic: hostile EVE timestamps near the
/// chrono range limits must produce an error, never a panic.
fn checked(
    datetime: chrono::DateTime<chrono::FixedOffset>,
    delta: chrono::Duration,
) -> Result<chrono::DateTime<chrono::FixedOffset>> {
    datetime
        .checked_add_signed(delta)
        .ok_or_else(|| anyhow!("timestamp out of range"))
}

/// Derive a capture window from an EVE event.
pub(crate) fn derive_window(event: &serde_json::Value) -> Result<Window> {
    let event_type = event["event_type"].as_str().unwrap_or("");
    let flow = if event_type == "netflow" {
        &event["netflow"]
    } else {
        &event["flow"]
    };
    let flow_start = flow["start"].as_str().map(parse_ts).transpose()?;

    let one_sec = chrono::Duration::seconds(1);
    let one_min = chrono::Duration::minutes(1);

    let (start, end) = match (event_type, flow_start) {
        ("flow" | "netflow", Some(flow_start)) => {
            let end = match flow["end"].as_str() {
                Some(flow_end) => checked(parse_ts(flow_end)?.datetime, one_sec)?,
                None => checked(flow_start.datetime, one_min)?,
            };
            (checked(flow_start.datetime, -one_sec)?, end)
        }
        (_, Some(flow_start)) => {
            let timestamp = event_timestamp(event)?;
            (
                checked(flow_start.datetime, -one_sec)?,
                checked(timestamp.datetime, one_min)?,
            )
        }
        (_, None) => {
            let timestamp = event_timestamp(event)?;
            (
                checked(timestamp.datetime, -one_min)?,
                checked(timestamp.datetime, one_min)?,
            )
        }
    };

    window(start, end)
}

fn window(
    start: chrono::DateTime<chrono::FixedOffset>,
    end: chrono::DateTime<chrono::FixedOffset>,
) -> Result<Window> {
    if end < start {
        bail!("capture window ends before it starts");
    }

    Ok(Window {
        start: start.into(),
        end: end.into(),
    })
}

/// A free-form window: `[start, start + span]`.
/// Used by the API's free-form (PCAP Filter / standalone) mode.
pub(crate) fn window_from_start(start: &DateTime, span: chrono::Duration) -> Result<Window> {
    let end = checked(start.datetime, span)?;
    window(start.datetime, end)
}

/// An event-relative window: `[event_ts − before, event_ts + after]`,
/// used by the API's event-relative mode.
pub(crate) fn window_around_event(
    event: &serde_json::Value,
    before: chrono::Duration,
    after: chrono::Duration,
) -> Result<Window> {
    let center = event_timestamp(event)?;
    let start = checked(center.datetime, -before)?;
    let end = checked(center.datetime, after)?;
    window(start, end)
}

/// EVE `proto` string to IP protocol number. Family-aware for bare
/// "icmp". Unknown names are an error — never an unmatchable filter.
pub(crate) fn normalize_proto(proto: &str, v6: bool) -> Result<u8> {
    let proto = proto.trim();
    if let Ok(number) = proto.parse::<u8>() {
        return Ok(number);
    }
    let number = match proto.to_ascii_lowercase().as_str() {
        "tcp" => 6,
        "udp" => 17,
        "sctp" => 132,
        "gre" => 47,
        "esp" => 50,
        "ah" => 51,
        "ipip" => 4,
        "ipv6" => 41,
        "icmp" => {
            if v6 {
                58
            } else {
                1
            }
        }
        "ipv6-icmp" | "icmpv6" => 58,
        _ => bail!("unsupported protocol: {proto}"),
    };
    Ok(number)
}

const PORT_PROTOCOLS: [u8; 3] = [6, 17, 132];

/// Read an optional transport port. Missing and null fields mean that the
/// event did not report a port; any other present value must be a valid
/// unsigned 16-bit integer so malformed data cannot broaden the filter.
fn event_port(event: &serde_json::Value, field: &str) -> Result<Option<u16>> {
    match event.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            let port = value
                .as_u64()
                .ok_or_else(|| anyhow!("event has invalid {field}"))?;
            let port = u16::try_from(port).map_err(|_| anyhow!("event has invalid {field}"))?;
            Ok(Some(port))
        }
    }
}

/// Derive the flow selector from a stored EVE event. Ports are only
/// used for port-carrying protocols.
pub(crate) fn selector_from_event(event: &serde_json::Value) -> Result<FlowSelector> {
    let src_ip: IpAddr = event["src_ip"]
        .as_str()
        .ok_or_else(|| anyhow!("event has no src_ip"))?
        .parse()?;
    let dest_ip: IpAddr = event["dest_ip"]
        .as_str()
        .ok_or_else(|| anyhow!("event has no dest_ip"))?
        .parse()?;
    if src_ip.is_ipv4() != dest_ip.is_ipv4() {
        // A mixed-family selector can never match a packet.
        bail!("mixed address families: {src_ip} and {dest_ip}");
    }
    let proto = event["proto"]
        .as_str()
        .ok_or_else(|| anyhow!("event has no proto"))?;
    let proto = normalize_proto(proto, src_ip.is_ipv6())?;
    let (src_port, dest_port) = if PORT_PROTOCOLS.contains(&proto) {
        (
            event_port(event, "src_port")?,
            event_port(event, "dest_port")?,
        )
    } else {
        (None, None)
    };
    Ok(FlowSelector {
        proto,
        a: (src_ip, src_port),
        b: (dest_ip, dest_port),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    fn seconds(window: &Window) -> (i64, i64) {
        (window.start.to_seconds(), window.end.to_seconds())
    }

    #[test]
    fn test_flow_window() {
        let event = json!({
            "event_type": "flow",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:00:00.000000+0000",
                "end": "2026-07-03T10:05:00.000000+0000",
            }
        });
        let window = derive_window(&event).unwrap();
        let start = window.start.to_seconds();
        let end = window.end.to_seconds();
        assert_eq!(end - start, 300 + 2);
    }

    #[test]
    fn test_flow_window_rejects_end_before_start() {
        let event = json!({
            "event_type": "flow",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:05:00.000000+0000",
                "end": "2026-07-03T10:00:00.000000+0000",
            }
        });
        assert!(derive_window(&event).is_err());
    }

    #[test]
    fn test_alert_window_rejects_event_before_flow_start() {
        let event = json!({
            "event_type": "alert",
            "timestamp": "2026-07-03T10:00:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:10:00.000000+0000",
            }
        });
        assert!(derive_window(&event).is_err());
    }

    #[test]
    fn test_netflow_window() {
        let event = json!({
            "event_type": "netflow",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
            "netflow": {
                "start": "2026-07-03T10:00:00.000000+0000",
                "end": "2026-07-03T10:01:00.000000+0000",
            }
        });
        let window = derive_window(&event).unwrap();
        let (start, end) = seconds(&window);
        assert_eq!(end - start, 60 + 2);
    }

    #[test]
    fn test_flow_window_missing_end() {
        let event = json!({
            "event_type": "flow",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:00:00.000000+0000",
            }
        });
        let window = derive_window(&event).unwrap();
        let (start, end) = seconds(&window);
        assert_eq!(end - start, 61);
    }

    #[test]
    fn test_alert_window_uses_flow_start() {
        // Alert 10 minutes into the flow: window runs from just
        // before the flow start to a minute past the alert.
        let event = json!({
            "event_type": "alert",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:00:00.000000+0000",
            }
        });
        let window = derive_window(&event).unwrap();
        let (start, end) = seconds(&window);
        assert_eq!(end - start, 1 + 600 + 60);
    }

    #[test]
    fn test_timestamp_only_window() {
        let event = json!({
            "event_type": "dns",
            "timestamp": "2026-07-03T10:10:00.000000+0000",
        });
        let window = derive_window(&event).unwrap();
        let (start, end) = seconds(&window);
        assert_eq!(end - start, 120);
    }

    #[test]
    fn test_long_flow_window_is_not_clamped() {
        let event = json!({
            "event_type": "flow",
            "timestamp": "2026-07-03T20:00:00.000000+0000",
            "flow": {
                "start": "2026-07-03T10:00:00.000000+0000",
                "end": "2026-07-03T20:00:00.000000+0000",
            }
        });
        let window = derive_window(&event).unwrap();
        let (start, end) = seconds(&window);
        assert_eq!(end - start, 10 * 3600 + 2);
        let flow_start = datetime::parse("2026-07-03T10:00:00.000000+0000", None).unwrap();
        assert_eq!(start, flow_start.to_seconds() - 1);
    }

    // Both the old EVE offset format and RFC3339 must parse.
    #[test]
    fn test_old_eve_timestamp_offsets() {
        for timestamp in [
            "2022-04-12T17:20:42.294911-0600",
            "2022-04-12T17:20:42.294911-06:00",
        ] {
            let event = json!({
                "event_type": "dns",
                "timestamp": timestamp,
            });
            let window = derive_window(&event).unwrap();
            let (start, end) = seconds(&window);
            assert_eq!(end - start, 120, "timestamp {timestamp}");
        }
    }

    #[test]
    fn test_bad_timestamp_is_error() {
        let event = json!({
            "event_type": "dns",
            "timestamp": "not a timestamp",
        });
        assert!(derive_window(&event).is_err());
    }

    #[test]
    fn test_hostile_timestamps_error_not_panic() {
        // chrono parses extended years right up to its range limits;
        // window arithmetic within a minute of the boundary must
        // error, never panic.
        let near_min =
            (chrono::DateTime::<chrono::Utc>::MIN_UTC + chrono::Duration::seconds(30)).to_rfc3339();
        let near_max =
            (chrono::DateTime::<chrono::Utc>::MAX_UTC - chrono::Duration::seconds(30)).to_rfc3339();
        for timestamp in [near_min, near_max] {
            let event = json!({
                "event_type": "dns",
                "timestamp": timestamp,
            });
            assert!(derive_window(&event).is_err(), "{timestamp}");
        }
    }

    #[test]
    fn test_normalize_proto() {
        assert_eq!(normalize_proto("TCP", false).unwrap(), 6);
        assert_eq!(normalize_proto("tcp", false).unwrap(), 6);
        assert_eq!(normalize_proto("udp", false).unwrap(), 17);
        assert_eq!(normalize_proto("sctp", false).unwrap(), 132);
        assert_eq!(normalize_proto("gre", false).unwrap(), 47);
        assert_eq!(normalize_proto("esp", false).unwrap(), 50);
        assert_eq!(normalize_proto("ah", false).unwrap(), 51);
        assert_eq!(normalize_proto("ipip", false).unwrap(), 4);
        assert_eq!(normalize_proto("ipv6", false).unwrap(), 41);
        // Family aware ICMP.
        assert_eq!(normalize_proto("ICMP", false).unwrap(), 1);
        assert_eq!(normalize_proto("icmp", true).unwrap(), 58);
        assert_eq!(normalize_proto("IPV6-ICMP", false).unwrap(), 58);
        assert_eq!(normalize_proto("icmpv6", false).unwrap(), 58);
        // Numeric passthrough.
        assert_eq!(normalize_proto("47", false).unwrap(), 47);
        // Unknown: an error, never an unmatchable filter.
        assert!(normalize_proto("quic", false).is_err());
        assert!(normalize_proto("", false).is_err());
        assert!(normalize_proto("300", false).is_err());
    }

    #[test]
    fn test_selector_from_event() {
        let event = json!({
            "proto": "TCP",
            "src_ip": "10.1.1.5",
            "src_port": 44212,
            "dest_ip": "192.0.2.10",
            "dest_port": 443,
        });
        let selector = selector_from_event(&event).unwrap();
        assert_eq!(selector.proto, 6);
        assert_eq!(selector.a, ("10.1.1.5".parse().unwrap(), Some(44212)));
        assert_eq!(selector.b, ("192.0.2.10".parse().unwrap(), Some(443)));
    }

    #[test]
    fn test_selector_transport_ports_may_be_missing_or_null() {
        let event = json!({
            "proto": "TCP",
            "src_ip": "10.1.1.5",
            "src_port": null,
            "dest_ip": "192.0.2.10",
        });
        let selector = selector_from_event(&event).unwrap();
        assert_eq!(selector.a.1, None);
        assert_eq!(selector.b.1, None);
    }

    #[test]
    fn test_selector_rejects_invalid_transport_ports() {
        for invalid in [json!(65536), json!(-1), json!("443")] {
            let mut event = json!({
                "proto": "TCP",
                "src_ip": "10.1.1.5",
                "src_port": 44212,
                "dest_ip": "192.0.2.10",
                "dest_port": 443,
            });
            event["src_port"] = invalid;
            assert!(selector_from_event(&event).is_err());
        }

        let event = json!({
            "proto": "UDP",
            "src_ip": "10.1.1.5",
            "src_port": 53,
            "dest_ip": "192.0.2.10",
            "dest_port": 70000,
        });
        assert!(selector_from_event(&event).is_err());
    }

    #[test]
    fn test_selector_portless_drops_ports() {
        // Suricata puts ICMP type/code in the port fields of some
        // outputs; they must not become port filters.
        let event = json!({
            "proto": "ICMP",
            "src_ip": "10.1.1.5",
            "src_port": 8,
            "dest_ip": "192.0.2.10",
            "dest_port": 0,
        });
        let selector = selector_from_event(&event).unwrap();
        assert_eq!(selector.proto, 1);
        assert_eq!(selector.a.1, None);
        assert_eq!(selector.b.1, None);
    }

    #[test]
    fn test_selector_icmpv6_family() {
        let event = json!({
            "proto": "ICMP",
            "src_ip": "2001:db8::1",
            "dest_ip": "2001:db8::2",
        });
        let selector = selector_from_event(&event).unwrap();
        assert_eq!(selector.proto, 58);
    }

    #[test]
    fn test_selector_unknown_proto() {
        let event = json!({
            "proto": "quic",
            "src_ip": "10.1.1.5",
            "dest_ip": "192.0.2.10",
        });
        assert!(selector_from_event(&event).is_err());
    }

    #[test]
    fn test_selector_mixed_family_rejected() {
        // A mixed-family selector could never match a packet; refuse
        // to build one.
        let event = json!({
            "proto": "TCP",
            "src_ip": "10.1.1.5",
            "src_port": 44212,
            "dest_ip": "2001:db8::1",
            "dest_port": 443,
        });
        assert!(selector_from_event(&event).is_err());
    }
}
