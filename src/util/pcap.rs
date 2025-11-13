// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! PCAP file generation and EVE to PCAP conversion utilities.
//! Includes packet building to convert Eve payload's into a packet.
//!
//! TODO: Ipv6

use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use base64::prelude::*;
use bytes::{Buf, BufMut, BytesMut};
use tracing::warn;

use crate::datetime::DateTime;
use crate::eve::Eve;

// PCAP file constants and sizes
const MAGIC: u32 = 0xa1b2_c3d4;
const VERSION_MAJOR: u16 = 2;
const VERSION_MINOR: u16 = 4;

pub(crate) const FILE_HEADER_LEN: usize = 24;
const PACKET_HEADER_LEN: usize = 16;
const PCAP_RECORD_HEADER_SIZE: usize = 16;

/// Link types for PCAP files
#[repr(C)]
pub(crate) enum LinkType {
    Ethernet = 1,
    Raw = 101,
}

//
// Packet building structures and functions
//

#[derive(Debug, Clone)]
pub(crate) enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    pub(crate) fn from_name(proto: &str) -> Option<Protocol> {
        let proto = proto.to_lowercase();
        match proto.as_str() {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

impl From<Protocol> for u8 {
    fn from(p: Protocol) -> Self {
        match p {
            Protocol::Tcp => 6,
            Protocol::Udp => 17,
        }
    }
}

pub(crate) struct UdpBuilder {
    source_port: u16,
    destination_port: u16,
    payload: Option<Vec<u8>>,
}

impl UdpBuilder {
    pub(crate) fn new(source_port: u16, destination_port: u16) -> Self {
        Self {
            source_port,
            destination_port,
            payload: None,
        }
    }

    pub(crate) fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    pub(crate) fn build(self) -> Vec<u8> {
        let payload = self.payload.unwrap_or_default();
        let mut buf = BytesMut::new();
        buf.put_u16(self.source_port);
        buf.put_u16(self.destination_port);
        buf.put_u16(8 + payload.len() as u16);
        buf.put_u16(0);
        buf.extend_from_slice(&payload);

        buf.to_vec()
    }
}

pub(crate) struct TcpBuilder {
    source_port: u16,
    destination_port: u16,
    payload: Vec<u8>,
}

impl TcpBuilder {
    pub(crate) fn new(source_port: u16, destination_port: u16) -> Self {
        Self {
            source_port,
            destination_port,
            payload: vec![],
        }
    }

    pub(crate) fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub(crate) fn build(self) -> Vec<u8> {
        let mut buf = BytesMut::new();

        buf.put_u16(self.source_port);
        buf.put_u16(self.destination_port);

        buf.put_u32(0); // Sequence number
        buf.put_u32(0); // Acknowledgement number

        let flags = 5 << 12;
        buf.put_u16(flags);

        buf.put_u16(1024); // Window size

        buf.put_u16(0); // Checksum
        buf.put_u16(0); // Urgent pointer

        buf.extend_from_slice(&self.payload);

        buf.to_vec()
    }
}

pub(crate) struct Ip4Builder {
    source_addr: Ipv4Addr,
    dest_addr: Ipv4Addr,
    protocol: Protocol,
    ttl: u8,
    payload: Vec<u8>,
}

impl Default for Ip4Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Ip4Builder {
    pub(crate) fn new() -> Self {
        Self {
            source_addr: Ipv4Addr::new(0, 0, 0, 0),
            dest_addr: Ipv4Addr::new(0, 0, 0, 0),
            ttl: 255,
            protocol: Protocol::Udp,
            payload: vec![],
        }
    }

    pub(crate) fn source(mut self, addr: Ipv4Addr) -> Self {
        self.source_addr = addr;
        self
    }

    pub(crate) fn destination(mut self, addr: Ipv4Addr) -> Self {
        self.dest_addr = addr;
        self
    }

    pub(crate) fn protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub(crate) fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub(crate) fn build(self) -> Vec<u8> {
        let mut buf = BytesMut::new();

        buf.put_u16(0x4500); // Version(4) + IHL(4) + DSCP + ECN
        buf.put_u16(20 + self.payload.len() as u16); // Total length (header + payload)

        buf.put_u32(0); // ID(2 bytes), Flags(3 bits), Fragment offset (13 bits)

        buf.put_u8(self.ttl);
        buf.put_u8(self.protocol.clone().into());

        buf.put_u16(0x0000); // Checksum.

        buf.extend_from_slice(&self.source_addr.octets());
        buf.extend_from_slice(&self.dest_addr.octets());

        buf.extend(&self.payload);

        let mut out = buf.to_vec();

        let csum = &self.ip_checksum(&out);
        out[10] = ((csum >> 8) & 0xffu16) as u8;
        out[11] = (csum & 0xff) as u8;

        if let Protocol::Udp = &self.protocol {
            let csum = &self.tcpudp_checksum(&out, Protocol::Udp);
            out[26] = ((csum >> 8) & 0xffu16) as u8;
            out[27] = (csum & 0xff) as u8;
        } else if let Protocol::Tcp = &self.protocol {
            let csum = &self.tcpudp_checksum(&out, Protocol::Tcp);
            out[36] = ((csum >> 8) & 0xffu16) as u8;
            out[37] = (csum & 0xff) as u8;
        }

        out
    }

    fn ip_checksum(&self, input: &[u8]) -> u16 {
        let mut result = 0xffffu32;
        let mut hdr = &input[0..20];
        while hdr.remaining() > 0 {
            result += hdr.get_u16() as u32;
            if result > 0xffff {
                result -= 0xffff;
            }
        }
        !result as u16
    }

    fn tcpudp_checksum(&self, input: &[u8], proto: Protocol) -> u16 {
        let mut result = 0xffffu32;
        let mut pseudo = BytesMut::new();
        pseudo.extend_from_slice(&input[12..20]);
        pseudo.put_u8(0);
        pseudo.put_u8(proto.into());
        pseudo.put_u16(input.len() as u16 - 20_u16);

        while pseudo.remaining() > 0 {
            result += pseudo.get_u16() as u32;
            if result > 0xffff {
                result -= 0xffff;
            }
        }

        let mut pdu = &input[20..];
        loop {
            let remaining = pdu.remaining();
            if remaining == 0 {
                break;
            }
            if remaining == 1 {
                result += (pdu.get_u8() as u32) << 8;
            } else {
                result += pdu.get_u16() as u32;
            }
            if result > 0xffff {
                result -= 0xffff;
            }
        }

        !result as u16
    }
}

//
// PCAP File Generation Functions
//

/// Creates a PCAP file header
pub(crate) fn create_header(linktype: u32) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(FILE_HEADER_LEN);

    // Write out the file header.
    buf.put_u32_le(MAGIC);
    buf.put_u16_le(VERSION_MAJOR);
    buf.put_u16_le(VERSION_MINOR);
    buf.put_u32_le(0); // This zone (GMT to local correction)
    buf.put_u32_le(0); // Accuracy of timestamps (sigfigs)
    buf.put_u32_le(0xFFFF_FFFF); // Snap length (max value)
    buf.put_u32_le(linktype); // Data link type

    buf.to_vec()
}

/// Creates a PCAP packet record (header + data).
pub(crate) fn create_record(ts: DateTime, packet: &[u8]) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(PCAP_RECORD_HEADER_SIZE + packet.len());

    // The record header.
    // FIXME: Should this really be nanos?
    buf.put_u32_le(ts.to_nanos() as u32); // ts_sec
    buf.put_u32_le(ts.micros_part() as u32); // ts_usec
    buf.put_u32_le(packet.len() as u32); // incl_len (captured length)
    buf.put_u32_le(packet.len() as u32); // orig_len (actual length)
    buf.put_slice(packet);

    buf.to_vec()
}

/// Create a complete PCAP file with a single packet
/// This is a convenience function that calls create_header and create_record
pub(crate) fn create(linktype: u32, ts: DateTime, packet: &[u8]) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(FILE_HEADER_LEN + PACKET_HEADER_LEN + packet.len());

    let header = create_header(linktype);
    let record = create_record(ts, packet);

    buf.put_slice(&header);
    buf.put_slice(&record);

    buf.to_vec()
}

/// Construct a packet from EVE event payload
pub(crate) fn packet_from_payload(event: &serde_json::Value) -> Result<Vec<u8>> {
    let payload = &event["payload"]
        .as_str()
        .ok_or_else(|| anyhow!("no payload field"))?;
    let payload = BASE64_STANDARD.decode(payload)?;

    let proto = &event["proto"]
        .as_str()
        .ok_or_else(|| anyhow!("no proto field"))?;
    let proto = Protocol::from_name(proto).ok_or_else(|| anyhow!("invalid protocol {}", proto))?;

    let src_ip = &event["src_ip"]
        .as_str()
        .ok_or_else(|| anyhow!("no src_ip field"))?;
    let src_ip = src_ip.parse::<IpAddr>()?;

    let dest_ip = &event["dest_ip"]
        .as_str()
        .ok_or_else(|| anyhow!("no dest_ip field"))?;
    let dest_ip = dest_ip.parse::<IpAddr>()?;

    match proto {
        Protocol::Tcp => {
            let src_port = &event["src_port"]
                .as_u64()
                .ok_or(anyhow!("invalid source port"))?;
            let dest_port = &event["dest_port"]
                .as_u64()
                .ok_or(anyhow!("invalid destination port"))?;
            match (src_ip, dest_ip) {
                (IpAddr::V4(src), IpAddr::V4(dst)) => {
                    let tcp = TcpBuilder::new(*src_port as u16, *dest_port as u16)
                        .payload(payload)
                        .build();
                    let packet = Ip4Builder::new()
                        .source(src)
                        .destination(dst)
                        .protocol(proto)
                        .payload(tcp)
                        .build();
                    Ok(packet)
                }
                (IpAddr::V6(_), _) | (_, IpAddr::V6(_)) => bail!("ipv6 not supported"),
            }
        }
        Protocol::Udp => {
            let src_port = &event["src_port"]
                .as_u64()
                .ok_or(anyhow!("invalid source port"))?;
            let dest_port = &event["dest_port"]
                .as_u64()
                .ok_or(anyhow!("invalid destination port"))?;
            match (src_ip, dest_ip) {
                (IpAddr::V4(src), IpAddr::V4(dst)) => {
                    let udp = UdpBuilder::new(*src_port as u16, *dest_port as u16)
                        .payload(payload)
                        .build();
                    let packet = Ip4Builder::new()
                        .source(src)
                        .destination(dst)
                        .protocol(proto)
                        .payload(udp)
                        .build();
                    Ok(packet)
                }
                (IpAddr::V6(_), _) | (_, IpAddr::V6(_)) => bail!("ipv6 not supported"),
            }
        }
    }
}

/// Convert an EVE packet to PCAP data
pub(crate) fn packet_to_pcap(event: &serde_json::Value) -> Result<Vec<u8>> {
    let linktype = if let Some(linktype) = &event["packet_info"]["linktype"].as_u64() {
        *linktype as u32
    } else {
        warn!("No usable link-type in event, will use ethernet");
        LinkType::Ethernet as u32
    };

    let packet = &event["packet"]
        .as_str()
        .map(|s| BASE64_STANDARD.decode(s))
        .ok_or_else(|| anyhow!("no packet in event".to_string()))?
        .map_err(|err| anyhow!(format!("failed to base64 decode packet: {err}")))?;

    let ts = event
        .datetime()
        .ok_or_else(|| anyhow!("bad or missing timestamp field".to_string()))?;

    let pcap_buffer = create(linktype, ts, packet);
    Ok(pcap_buffer)
}

/// Convert an EVE payload to PCAP data
pub(crate) fn payload_to_pcap(event: &serde_json::Value) -> Result<Vec<u8>> {
    let ts = event
        .datetime()
        .ok_or_else(|| anyhow!("bad or missing timestamp field".to_string()))?;
    let packet = packet_from_payload(event)?;
    let pcap_buffer = create(LinkType::Raw as u32, ts, &packet);
    Ok(pcap_buffer)
}

/// Convert an EVE event to PCAP data based on the specified type
pub(crate) fn eve_to_pcap(event_type: &str, event: &serde_json::Value) -> Result<Vec<u8>> {
    match event_type {
        "packet" => packet_to_pcap(event),
        "payload" => payload_to_pcap(event),
        _ => bail!("invalid event type"),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_ip_addr() {
        let source: Ipv4Addr = "10.10.10.10".parse().unwrap();
        let destination: Ipv4Addr = "127.127.127.127".parse().unwrap();

        let tcp = TcpBuilder::new(5555, 6666)
            .payload(vec![0x41, 0x41, 0x41, 0x41, 0x41])
            .build();

        let builder = Ip4Builder::new()
            .protocol(Protocol::Tcp)
            .source(source)
            .destination(destination)
            .payload(tcp);

        let packet = builder.build();
        let now = DateTime::now();
        let _pcap_buffer = create(LinkType::Raw as u32, now, &packet);
    }

    #[test]
    fn test_pcap_from_packet() {
        let linktype = LinkType::Ethernet;
        let eve_timestamp = "2020-05-01T08:50:23.297919-0600";
        let packet_base64 =
            "oDafTEwo2MuK7aFGCABFAAAobXBAAEAGMBcKEAELzE/F3qMmAbtL2EhIK8jWtFAQAenVIwAAAAAAAAAA";
        let ts = crate::datetime::parse(eve_timestamp, None).unwrap();
        let packet = BASE64_STANDARD.decode(packet_base64).unwrap();
        let _pcap_buffer = super::create(linktype as u32, ts, &packet);
    }

    #[test]
    fn test_pcap_from_payload() {
        let event: serde_json::Value = serde_json::from_str(TEST_EVE_RECORD).unwrap();
        let packet = super::packet_from_payload(&event).unwrap();
        let ts = event.datetime().unwrap();
        let _pcap_buffer = super::create(LinkType::Raw as u32, ts, &packet);
    }

    const TEST_EVE_RECORD: &str = r#"
{
    "@timestamp": "2020-05-01T13:13:37.621Z",
    "alert": {
      "action": "allowed",
      "category": "A Network Trojan was detected",
      "gid": 1,
      "metadata": {
        "created_at": [
          "2015_10_06"
        ],
        "updated_at": [
          "2015_10_06"
        ]
      },
      "rev": 1,
      "severity": 1,
      "signature": "ET MALWARE ELF/muBoT IRC Activity 4",
      "signature_id": 2021915
    },
    "community_id": "1:Fc54mFg4nYz5CcocWFqQcWc38po=",
    "dest_ip": "10.16.1.10",
    "dest_port": 801,
    "evebox": {
      "filename": "/var/log/suricata/eve.json"
    },
    "event_type": "alert",
    "flow": {
      "bytes_toclient": 44205860244,
      "bytes_toserver": 56980285098,
      "pkts_toclient": 45765558,
      "pkts_toserver": 45613601,
      "start": "2020-04-30T22:59:51.502309-0600"
    },
    "flow_id": 2195577295579685,
    "host": "server1.unx.ca",
    "in_iface": "eno1",
    "metadata": {
      "flowbits": [
        "ET.HB.Response.CI",
        "ET.HB.Response.SI"
      ]
    },
    "packet": "bDvlJzW6ABEyF0nwCABFAAXcU/VAAEAGyvkKEAEEChABCggBAyG2gRJvgUzqtYAQB9OhogAAAQEICgDk/MxsMrfsfGdvbGFudHVzLmNvbSI7IGRpc3RhbmNlOjE7IHdpdGhpbjoxMzsgcmVmZXJlbmNlOnVybCxzc2xibC5hYnVzZS5jaDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTE7IHJldjoyOyBtZXRhZGF0YTphdHRhY2tfdGFyZ2V0IENsaWVudF9FbmRwb2ludCwgZGVwbG95bWVudCBQZXJpbWV0ZXIsIHRhZyBTU0xfTWFsaWNpb3VzX0NlcnQsIHNpZ25hdHVyZV9zZXZlcml0eSBNYWpvciwgY3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTZfMDdfMDE7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAxIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfFByaXZ8MjB8VmVyc2lvbiI7IGZhc3RfcGF0dGVybjsgZGlzdGFuY2U6MDsgcmVmZXJlbmNlOnVybCxwYXN0ZWJpbi5jb20vRUgxU0g5YUw7IGNsYXNzdHlwZTp0cm9qYW4tYWN0aXZpdHk7IHNpZDoyMDIxOTEyOyByZXY6MTsgbWV0YWRhdGE6Y3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTVfMTBfMDY7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAyIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfHNheXN8MjB8IjsgZmFzdF9wYXR0ZXJuOyBkaXN0YW5jZTowOyByZWZlcmVuY2U6dXJsLHBhc3RlYmluLmNvbS9FSDFTSDlhTDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTM7IHJldjoxOyBtZXRhZGF0YTpjcmVhdGVkX2F0IDIwMTVfMTBfMDYsIHVwZGF0ZWRfYXQgMjAxNV8xMF8wNjspCmFsZXJ0IHRjcCBhbnkgYW55IC0+IGFueSBhbnkgKG1zZzoiRVQgTUFMV0FSRSBFTEYvbXVCb1QgSVJDIEFjdGl2aXR5IDMiOyBmbG93OmVzdGFibGlzaGVkLGZyb21fc2VydmVyOyBjb250ZW50OiJOT1RJQ0UiOyBjb250ZW50OiJ8M2F8W0FwYWNoZSAvIFBIUCA1LngiOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGViaW4uY29tL0VIMVNIOWFMOyBjbGFzc3R5cGU6dHJvamFuLWFjdGl2aXR5OyBzaWQ6MjAyMTkxNDsgcmV2OjE7IG1ldGFkYXRhOmNyZWF0ZWRfYXQgMjAxNV8xMF8wNiwgdXBkYXRlZF9hdCAyMDE1XzEwXzA2OykKYWxlcnQgdGNwIGFueSBhbnkgLT4gYW55IGFueSAobXNnOiJFVCBNQUxXQVJFIEVMRi9tdUJvVCBJUkMgQWN0aXZpdHkgNCI7IGZsb3c6ZXN0YWJsaXNoZWQsZnJvbV9zZXJ2ZXI7IGNvbnRlbnQ6Ik5PVElDRSI7IGNvbnRlbnQ6IkZMT09EIDx0YXJnZXQ+IDxwb3J0PiA8c2Vjcz4iOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGU=",
    "payload": "fGdvbGFudHVzLmNvbSI7IGRpc3RhbmNlOjE7IHdpdGhpbjoxMzsgcmVmZXJlbmNlOnVybCxzc2xibC5hYnVzZS5jaDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTE7IHJldjoyOyBtZXRhZGF0YTphdHRhY2tfdGFyZ2V0IENsaWVudF9FbmRwb2ludCwgZGVwbG95bWVudCBQZXJpbWV0ZXIsIHRhZyBTU0xfTWFsaWNpb3VzX0NlcnQsIHNpZ25hdHVyZV9zZXZlcml0eSBNYWpvciwgY3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTZfMDdfMDE7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAxIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfFByaXZ8MjB8VmVyc2lvbiI7IGZhc3RfcGF0dGVybjsgZGlzdGFuY2U6MDsgcmVmZXJlbmNlOnVybCxwYXN0ZWJpbi5jb20vRUgxU0g5YUw7IGNsYXNzdHlwZTp0cm9qYW4tYWN0aXZpdHk7IHNpZDoyMDIxOTEyOyByZXY6MTsgbWV0YWRhdGE6Y3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTVfMTBfMDY7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAyIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfHNheXN8MjB8IjsgZmFzdF9wYXR0ZXJuOyBkaXN0YW5jZTowOyByZWZlcmVuY2U6dXJsLHBhc3RlYmluLmNvbS9FSDFTSDlhTDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTM7IHJldjoxOyBtZXRhZGF0YTpjcmVhdGVkX2F0IDIwMTVfMTBfMDYsIHVwZGF0ZWRfYXQgMjAxNV8xMF8wNjspCmFsZXJ0IHRjcCBhbnkgYW55IC0+IGFueSBhbnkgKG1zZzoiRVQgTUFMV0FSRSBFTEYvbXVCb1QgSVJDIEFjdGl2aXR5IDMiOyBmbG93OmVzdGFibGlzaGVkLGZyb21fc2VydmVyOyBjb250ZW50OiJOT1RJQ0UiOyBjb250ZW50OiJ8M2F8W0FwYWNoZSAvIFBIUCA1LngiOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGViaW4uY29tL0VIMVNIOWFMOyBjbGFzc3R5cGU6dHJvamFuLWFjdGl2aXR5OyBzaWQ6MjAyMTkxNDsgcmV2OjE7IG1ldGFkYXRhOmNyZWF0ZWRfYXQgMjAxNV8xMF8wNiwgdXBkYXRlZF9hdCAyMDE1XzEwXzA2OykKYWxlcnQgdGNwIGFueSBhbnkgLT4gYW55IGFueSAobXNnOiJFVCBNQUxXQVJFIEVMRi9tdUJvVCBJUkMgQWN0aXZpdHkgNCI7IGZsb3c6ZXN0YWJsaXNoZWQsZnJvbV9zZXJ2ZXI7IGNvbnRlbnQ6Ik5PVElDRSI7IGNvbnRlbnQ6IkZMT09EIDx0YXJnZXQ+IDxwb3J0PiA8c2Vjcz4iOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGU=",
    "proto": "TCP",
    "src_ip": "10.16.1.4",
    "src_port": 2049,
    "stream": 0,
    "tags": [
      "evebox.elastic-import"
    ],
    "timestamp": "2020-05-01T07:13:37.621315-0600"
}
"#;
}
