// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Flow selector to BPF filter expression rendering.
//!
//! A [`FlowSelector`] describes a flow direction-symmetrically; its
//! [`FlowSelector::to_bpf`] rendering matches the flow's packets in
//! both directions, plus IPv4 fragment continuations. Wrapping the
//! rendering with [`vlan_wrapped`] additionally matches packets
//! behind a single VLAN tag on link types that support the `vlan`
//! keyword. IPv6 fragment continuations and deeper QinQ stacks are
//! documented non-matches.
//!
//! The IPv4 fragment-continuation clause is stateless, so it
//! over-matches: it admits non-first fragments of any flow of the
//! same protocol between the two hosts, in either direction,
//! including flows on other port pairs (such as the mirrored-port
//! flow the port clauses deliberately reject). Such stray
//! continuations appear in the export as unreassemblable fragments.

use std::net::IpAddr;

// The BPF renderings below feed the libpcap extraction path, which
// Windows builds omit; the selector itself stays portable for the wire.
#[cfg(not(windows))]
const IPPROTO_ICMP: u8 = 1;
#[cfg(not(windows))]
const IPPROTO_TCP: u8 = 6;
#[cfg(not(windows))]
const IPPROTO_UDP: u8 = 17;
#[cfg(not(windows))]
const IPPROTO_ICMPV6: u8 = 58;
#[cfg(not(windows))]
const IPPROTO_SCTP: u8 = 132;

/// Direction-symmetric flow filter: `a` and `b` match either way
/// around.
#[derive(Debug, Clone)]
pub(crate) struct FlowSelector {
    /// IP protocol number, normalized from the EVE `proto` string.
    pub(crate) proto: u8,
    /// Ports are None for portless protocols (ICMP etc.).
    pub(crate) a: (IpAddr, Option<u16>),
    pub(crate) b: (IpAddr, Option<u16>),
}

/// Wrap a BPF expression so it also matches packets behind a single
/// VLAN tag. Only valid on link types with `vlan` keyword support
/// (Ethernet and friends); on others (raw IP, loopback, Linux
/// cooked, ...) libpcap refuses to compile the wrapped form.
#[cfg(not(windows))]
pub(crate) fn vlan_wrapped(expr: &str) -> String {
    // The untagged copy must come first: the `vlan` keyword shifts
    // the offsets of everything after it in the expression.
    format!("({expr}) or (vlan and ({expr}))")
}

impl FlowSelector {
    /// Render the selector as a BPF filter expression (libpcap
    /// syntax). The rendering does not match VLAN-tagged packets;
    /// wrap it with [`vlan_wrapped`] on link types that support it.
    #[cfg(not(windows))]
    pub(crate) fn to_bpf(&self) -> String {
        let (a, x) = &self.a;
        let (b, y) = &self.b;
        let n = self.proto;
        let kw = match n {
            IPPROTO_TCP => Some("tcp"),
            IPPROTO_UDP => Some("udp"),
            IPPROTO_SCTP => Some("sctp"),
            _ => None,
        };
        // Non-first IPv4 fragments carry no layer 4 header, so port
        // clauses can never match them; match them on hosts and
        // protocol so fragmented flows still reassemble in
        // Wireshark. Stateless, so it over-matches; see the module
        // docs.
        let fragments = if a.is_ipv4() && b.is_ipv4() {
            Some(format!(
                "(ip proto {n} and host {a} and host {b} and (ip[6:2] & 0x1fff != 0))"
            ))
        } else {
            None
        };
        let with_fragments = |expr: String| match &fragments {
            Some(fragments) => format!("{expr} or {fragments}"),
            None => expr,
        };
        match (kw, x, y) {
            (Some(kw), Some(x), Some(y)) => {
                // Exact direction-symmetric pairing: the mirrored-port
                // flow (a:y <-> b:x) must not match.
                with_fragments(format!(
                    "({kw} and src host {a} and dst host {b} and src port {x} and dst port {y}) or ({kw} and src host {b} and dst host {a} and src port {y} and dst port {x})"
                ))
            }
            (Some(kw), Some(p), None) | (Some(kw), None, Some(p)) => {
                with_fragments(format!("({kw} and host {a} and host {b} and port {p})"))
            }
            (Some(kw), None, None) => format!("{kw} and host {a} and host {b}"),
            (None, _, _) => {
                // The keyword must agree with the address family:
                // `icmp6 and host <ipv4>` is unmatchable and
                // libpcap's optimizer rejects it outright
                // ("expression rejects all packets"). Contradictory
                // combinations do occur, e.g. an IPv4 flow whose EVE
                // proto string is "IPV6-ICMP".
                match (n, a.is_ipv6()) {
                    (IPPROTO_ICMP, false) => format!("icmp and host {a} and host {b}"),
                    (IPPROTO_ICMPV6, true) => format!("icmp6 and host {a} and host {b}"),
                    (_, false) => format!("ip proto {n} and host {a} and host {b}"),
                    (_, true) => format!("ip6 proto {n} and host {a} and host {b}"),
                }
            }
        }
    }
}

#[cfg(all(test, not(windows)))]
mod test {
    use super::*;
    use crate::pcap::testutil::{
        ipv4_fragment, ipv4_packet, ipv6_packet, ports, vlan_tag, write_raw_pcap_file,
    };
    use crate::pcap::{FetchError, PcapFilter, PcapRequest, PcapSource, SpoolConfig, fetch};

    fn selector(proto: u8, a: (&str, Option<u16>), b: (&str, Option<u16>)) -> FlowSelector {
        FlowSelector {
            proto,
            a: (a.0.parse().unwrap(), a.1),
            b: (b.0.parse().unwrap(), b.1),
        }
    }

    fn udp_selector() -> FlowSelector {
        selector(17, ("10.1.1.5", Some(4000)), ("192.0.2.10", Some(53)))
    }

    /// Run the selector as a Flow filter through fetch() over a
    /// single-file tempdir spool and return the match count.
    fn match_count(selector: &FlowSelector, packets: &[Vec<u8>]) -> u64 {
        let dir = tempfile::tempdir().unwrap();
        write_raw_pcap_file(&dir.path().join("log.pcap.1700000000"), packets);
        let spool = SpoolConfig::new(dir.path(), None);
        let request = PcapRequest {
            filter: Some(PcapFilter::Flow(selector.clone())),
            ..Default::default()
        };
        let mut out = vec![];
        let cancel = tokio_util::sync::CancellationToken::new();
        match fetch(&PcapSource::Spool(spool), &request, &mut out, &cancel) {
            Ok(stats) => stats.packets,
            Err(FetchError::NoMatch(_)) => 0,
            Err(err) => panic!("fetch failed: {err:?}"),
        }
    }

    #[test]
    fn test_vlan_wrapped() {
        assert_eq!(
            vlan_wrapped("udp and port 53"),
            "(udp and port 53) or (vlan and (udp and port 53))"
        );
    }

    #[test]
    fn test_bpf_tcp_ipv4_both_ports() {
        let s = selector(6, ("10.1.1.5", Some(4000)), ("192.0.2.10", Some(53)));
        let expected = "(tcp and src host 10.1.1.5 and dst host 192.0.2.10 and src port 4000 and dst port 53) \
                    or (tcp and src host 192.0.2.10 and dst host 10.1.1.5 and src port 53 and dst port 4000) \
                    or (ip proto 6 and host 10.1.1.5 and host 192.0.2.10 and (ip[6:2] & 0x1fff != 0))";
        assert_eq!(s.to_bpf(), expected);
    }

    #[test]
    fn test_bpf_udp_ipv6_both_ports() {
        // No IPv4 fragment-continuation clause for IPv6 addresses.
        let s = selector(17, ("2001:db8::1", Some(4000)), ("2001:db8::2", Some(53)));
        let expected = "(udp and src host 2001:db8::1 and dst host 2001:db8::2 and src port 4000 and dst port 53) \
                    or (udp and src host 2001:db8::2 and dst host 2001:db8::1 and src port 53 and dst port 4000)";
        assert_eq!(s.to_bpf(), expected);
    }

    #[test]
    fn test_bpf_sctp_ipv4_both_ports() {
        let s = selector(132, ("10.1.1.5", Some(5000)), ("192.0.2.10", Some(80)));
        let expected = "(sctp and src host 10.1.1.5 and dst host 192.0.2.10 and src port 5000 and dst port 80) \
                    or (sctp and src host 192.0.2.10 and dst host 10.1.1.5 and src port 80 and dst port 5000) \
                    or (ip proto 132 and host 10.1.1.5 and host 192.0.2.10 and (ip[6:2] & 0x1fff != 0))";
        assert_eq!(s.to_bpf(), expected);
    }

    #[test]
    fn test_bpf_missing_port() {
        // The port clause cannot match non-first IPv4 fragments, so
        // the one-port forms carry the fragment-continuation clause
        // just like the both-ports form.
        let s = selector(17, ("10.1.1.5", None), ("192.0.2.10", Some(53)));
        assert_eq!(
            s.to_bpf(),
            "(udp and host 10.1.1.5 and host 192.0.2.10 and port 53) \
             or (ip proto 17 and host 10.1.1.5 and host 192.0.2.10 and (ip[6:2] & 0x1fff != 0))"
        );
        let s = selector(17, ("10.1.1.5", Some(4000)), ("192.0.2.10", None));
        assert_eq!(
            s.to_bpf(),
            "(udp and host 10.1.1.5 and host 192.0.2.10 and port 4000) \
             or (ip proto 17 and host 10.1.1.5 and host 192.0.2.10 and (ip[6:2] & 0x1fff != 0))"
        );
        // No ports, no port clause: continuation fragments already
        // match on protocol and hosts, so no fragment clause either.
        let s = selector(17, ("10.1.1.5", None), ("192.0.2.10", None));
        assert_eq!(s.to_bpf(), "udp and host 10.1.1.5 and host 192.0.2.10");
    }

    #[test]
    fn test_bpf_missing_port_ipv6() {
        // No IPv4 fragment clause for IPv6 addresses.
        let s = selector(17, ("2001:db8::1", None), ("2001:db8::2", Some(53)));
        assert_eq!(
            s.to_bpf(),
            "(udp and host 2001:db8::1 and host 2001:db8::2 and port 53)"
        );
    }

    #[test]
    fn test_bpf_icmp() {
        let s = selector(1, ("10.1.1.5", None), ("192.0.2.10", None));
        assert_eq!(s.to_bpf(), "icmp and host 10.1.1.5 and host 192.0.2.10");
    }

    #[test]
    fn test_bpf_icmpv6() {
        let s = selector(58, ("2001:db8::1", None), ("2001:db8::2", None));
        assert_eq!(
            s.to_bpf(),
            "icmp6 and host 2001:db8::1 and host 2001:db8::2"
        );
    }

    #[test]
    fn test_bpf_icmp_family_contradiction() {
        // Suricata can emit an ICMP protocol number of the "wrong"
        // family, e.g. proto "IPV6-ICMP" (58) on an IPv4 flow. The
        // keyword must follow the address family: `icmp6 and host
        // <ipv4>` is unmatchable and rejected by the libpcap
        // optimizer.
        let v4 = selector(58, ("10.1.1.5", None), ("192.0.2.10", None));
        assert_eq!(
            v4.to_bpf(),
            "ip proto 58 and host 10.1.1.5 and host 192.0.2.10"
        );
        let v6 = selector(1, ("2001:db8::1", None), ("2001:db8::2", None));
        assert_eq!(
            v6.to_bpf(),
            "ip6 proto 1 and host 2001:db8::1 and host 2001:db8::2"
        );
        let dead = pcap::Capture::dead(pcap::Linktype::ETHERNET).unwrap();
        for s in [v4, v6] {
            let expr = vlan_wrapped(&s.to_bpf());
            dead.compile(&expr, true)
                .unwrap_or_else(|err| panic!("failed to compile {expr:?}: {err}"));
        }
    }

    #[test]
    fn test_bpf_other_proto() {
        let s = selector(47, ("10.1.1.5", None), ("192.0.2.10", None));
        assert_eq!(
            s.to_bpf(),
            "ip proto 47 and host 10.1.1.5 and host 192.0.2.10"
        );
        let s = selector(47, ("2001:db8::1", None), ("2001:db8::2", None));
        assert_eq!(
            s.to_bpf(),
            "ip6 proto 47 and host 2001:db8::1 and host 2001:db8::2"
        );
    }

    /// Every generated shape must compile with libpcap, both the
    /// base rendering and the VLAN-wrapped form used on Ethernet.
    #[test]
    fn test_bpf_compiles() {
        let selectors = [
            // tcp/udp/sctp, both ports, IPv4 and IPv6.
            selector(6, ("10.1.1.5", Some(4000)), ("192.0.2.10", Some(53))),
            selector(17, ("10.1.1.5", Some(4000)), ("192.0.2.10", Some(53))),
            selector(132, ("10.1.1.5", Some(4000)), ("192.0.2.10", Some(53))),
            selector(6, ("2001:db8::1", Some(4000)), ("2001:db8::2", Some(53))),
            selector(17, ("2001:db8::1", Some(4000)), ("2001:db8::2", Some(53))),
            selector(132, ("2001:db8::1", Some(4000)), ("2001:db8::2", Some(53))),
            // Missing ports.
            selector(17, ("10.1.1.5", None), ("192.0.2.10", Some(53))),
            selector(17, ("10.1.1.5", Some(4000)), ("192.0.2.10", None)),
            selector(17, ("10.1.1.5", None), ("192.0.2.10", None)),
            // ICMP both families.
            selector(1, ("10.1.1.5", None), ("192.0.2.10", None)),
            selector(58, ("2001:db8::1", None), ("2001:db8::2", None)),
            // ICMP protocol numbers contradicting the address family.
            selector(58, ("10.1.1.5", None), ("192.0.2.10", None)),
            selector(1, ("2001:db8::1", None), ("2001:db8::2", None)),
            // Numeric protocols, both families.
            selector(47, ("10.1.1.5", None), ("192.0.2.10", None)),
            selector(47, ("2001:db8::1", None), ("2001:db8::2", None)),
            selector(50, ("10.1.1.5", None), ("192.0.2.10", None)),
        ];
        let dead = pcap::Capture::dead(pcap::Linktype::ETHERNET).unwrap();
        for s in &selectors {
            for expr in [s.to_bpf(), vlan_wrapped(&s.to_bpf())] {
                dead.compile(&expr, true)
                    .unwrap_or_else(|err| panic!("failed to compile {expr:?}: {err}"));
            }
        }
    }

    #[test]
    fn test_match_direction_symmetry() {
        let s = udp_selector();
        let a_to_b = ipv4_packet(17, "10.1.1.5", "192.0.2.10", &ports(4000, 53));
        let b_to_a = ipv4_packet(17, "192.0.2.10", "10.1.1.5", &ports(53, 4000));
        assert_eq!(match_count(&s, &[a_to_b, b_to_a]), 2);
    }

    #[test]
    fn test_match_mirrored_port_flow_rejected() {
        // The same hosts with the ports swapped is a different flow.
        let s = udp_selector();
        let mirrored = ipv4_packet(17, "10.1.1.5", "192.0.2.10", &ports(53, 4000));
        let mirrored_reply = ipv4_packet(17, "192.0.2.10", "10.1.1.5", &ports(4000, 53));
        assert_eq!(match_count(&s, &[mirrored, mirrored_reply]), 0);
    }

    #[test]
    fn test_match_wrong_host_or_port_rejected() {
        let s = udp_selector();
        let wrong_host = ipv4_packet(17, "10.1.1.6", "192.0.2.10", &ports(4000, 53));
        let wrong_port = ipv4_packet(17, "10.1.1.5", "192.0.2.10", &ports(4001, 53));
        let wrong_proto = ipv4_packet(6, "10.1.1.5", "192.0.2.10", &ports(4000, 53));
        assert_eq!(match_count(&s, &[wrong_host, wrong_port, wrong_proto]), 0);
    }

    #[test]
    fn test_match_icmp_v4() {
        let s = selector(1, ("10.1.1.5", None), ("192.0.2.10", None));
        let echo = ipv4_packet(1, "10.1.1.5", "192.0.2.10", &[8, 0, 0, 0, 0, 0, 0, 0]);
        let reply = ipv4_packet(1, "192.0.2.10", "10.1.1.5", &[0, 0, 0, 0, 0, 0, 0, 0]);
        let other = ipv4_packet(1, "10.1.1.5", "203.0.113.7", &[8, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(match_count(&s, &[echo, reply, other]), 2);
    }

    #[test]
    fn test_match_icmpv6() {
        let s = selector(58, ("2001:db8::1", None), ("2001:db8::2", None));
        let echo = ipv6_packet(
            58,
            "2001:db8::1",
            "2001:db8::2",
            &[128, 0, 0, 0, 0, 0, 0, 0],
        );
        let reply = ipv6_packet(
            58,
            "2001:db8::2",
            "2001:db8::1",
            &[129, 0, 0, 0, 0, 0, 0, 0],
        );
        let other = ipv6_packet(
            58,
            "2001:db8::1",
            "2001:db8::3",
            &[128, 0, 0, 0, 0, 0, 0, 0],
        );
        assert_eq!(match_count(&s, &[echo, reply, other]), 2);
    }

    #[test]
    fn test_match_vlan_tagged() {
        let s = udp_selector();
        let tagged = vlan_tag(
            &ipv4_packet(17, "10.1.1.5", "192.0.2.10", &ports(4000, 53)),
            100,
        );
        let tagged_other = vlan_tag(
            &ipv4_packet(17, "10.1.1.6", "192.0.2.10", &ports(4000, 53)),
            100,
        );
        assert_eq!(match_count(&s, &[tagged, tagged_other]), 1);
    }

    #[test]
    fn test_match_numeric_proto_gre() {
        let s = selector(47, ("10.1.1.5", None), ("192.0.2.10", None));
        let gre = ipv4_packet(47, "10.1.1.5", "192.0.2.10", &[0, 0, 0x08, 0x00]);
        let other = ipv4_packet(47, "10.1.1.5", "203.0.113.7", &[0, 0, 0x08, 0x00]);
        assert_eq!(match_count(&s, &[gre, other]), 1);
    }

    #[test]
    fn test_match_sctp() {
        let s = selector(132, ("10.1.1.5", Some(5000)), ("192.0.2.10", Some(80)));
        let m = ipv4_packet(132, "10.1.1.5", "192.0.2.10", &ports(5000, 80));
        let mirrored = ipv4_packet(132, "10.1.1.5", "192.0.2.10", &ports(80, 5000));
        assert_eq!(match_count(&s, &[m, mirrored]), 1);
    }

    #[test]
    fn test_match_ipv4_fragment_continuation() {
        let s = udp_selector();
        // First fragment: more-fragments set, offset 0, has the UDP
        // header, matched by the port clause.
        let first = ipv4_fragment(17, "10.1.1.5", "192.0.2.10", 0x2000, &ports(4000, 53));
        // Continuation: non-zero offset, no layer 4 header, matched
        // by the fragment clause.
        let continuation = ipv4_fragment(17, "10.1.1.5", "192.0.2.10", 100, &[0xde; 8]);
        // A continuation between other hosts stays rejected.
        let other = ipv4_fragment(17, "10.1.1.6", "192.0.2.10", 100, &[0xde; 8]);
        assert_eq!(match_count(&s, &[first, continuation, other]), 2);
    }

    #[test]
    fn test_match_fragment_continuation_one_port() {
        // Fragment continuations must also match when only one port
        // is known.
        let s = selector(17, ("10.1.1.5", None), ("192.0.2.10", Some(53)));
        let first = ipv4_fragment(17, "10.1.1.5", "192.0.2.10", 0x2000, &ports(4000, 53));
        let continuation = ipv4_fragment(17, "10.1.1.5", "192.0.2.10", 100, &[0xde; 8]);
        let other = ipv4_fragment(17, "10.1.1.6", "192.0.2.10", 100, &[0xde; 8]);
        assert_eq!(match_count(&s, &[first, continuation, other]), 2);
    }
}
