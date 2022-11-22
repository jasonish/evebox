// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use bytes::{BufMut, BytesMut};
use std::net::IpAddr;

use crate::eve::eve::EveJson;
use crate::packet;

const MAGIC: u32 = 0xa1b2_c3d4;
const VERSION_MAJOR: u16 = 2;
const VERSION_MINOR: u16 = 4;

const FILE_HEADER_LEN: usize = 30;
const PACKET_HEADER_LEN: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("missing field: {0}")]
    MissingField(String),
    #[error("invalid protocol: {0}")]
    InvalidProto(String),
    #[error("failed to decode payload: {0}")]
    PayloadDecodeError(base64::DecodeError),
    #[error("bad address: {0}")]
    BadAddr(std::net::AddrParseError),
    #[error("mismatched ip address versions")]
    MissMatchedIpAddrVersions,
    #[error("invalid source port")]
    InvalidSourcePort,
    #[error("invalid destination port")]
    InvalidDestinationPort,
    #[error("IPv6 not supported")]
    Ipv6NotSupported,
    #[error("protocol not supported: {0}")]
    ProtocolNotSupported(String),
}

#[repr(C)]
pub enum LinkType {
    Null = 0,
    Ethernet = 1,
    Raw = 101,
}

impl LinkType {
    pub fn from(val: u8) -> Option<LinkType> {
        match val {
            0 => Some(Self::Null),
            1 => Some(Self::Ethernet),
            101 => Some(Self::Raw),
            _ => None,
        }
    }
}

pub fn packet_from_payload(event: &EveJson) -> Result<Vec<u8>, Error> {
    let payload = if let Some(payload) = &event["payload"].as_str() {
        base64::decode(payload).map_err(Error::PayloadDecodeError)?
    } else {
        return Err(Error::MissingField("payload".to_string()));
    };
    let proto = if let Some(proto) = &event["proto"].as_str() {
        packet::Protocol::from_name(proto)
            .ok_or_else(|| Error::InvalidProto((*proto).to_string()))?
    } else {
        return Err(Error::MissingField("proto".to_string()));
    };
    let src_ip = if let Some(src_ip) = &event["src_ip"].as_str() {
        src_ip.parse::<IpAddr>().map_err(Error::BadAddr)?
    } else {
        return Err(Error::MissingField("src_ip".to_string()));
    };
    let dest_ip = if let Some(dest_ip) = &event["dest_ip"].as_str() {
        dest_ip.parse::<IpAddr>().map_err(Error::BadAddr)?
    } else {
        return Err(Error::MissingField("dest_ip".to_string()));
    };

    match proto {
        packet::Protocol::Tcp => {
            let src_port = &event["src_port"]
                .as_u64()
                .ok_or(Error::InvalidDestinationPort)?;
            let dest_port = &event["dest_port"]
                .as_u64()
                .ok_or(Error::InvalidSourcePort)?;
            match (src_ip, dest_ip) {
                (IpAddr::V4(src), IpAddr::V4(dst)) => {
                    let tcp = packet::TcpBuilder::new(*src_port as u16, *dest_port as u16)
                        .payload(payload)
                        .build();
                    let packet = packet::Ip4Builder::new()
                        .source(src)
                        .destination(dst)
                        .protocol(proto)
                        .payload(tcp)
                        .build();
                    return Ok(packet);
                }
                (IpAddr::V6(_src), IpAddr::V6(_dst)) => {
                    return Err(Error::Ipv6NotSupported);
                }
                _ => {
                    return Err(Error::MissMatchedIpAddrVersions);
                }
            }
        }
        packet::Protocol::Udp => {
            let src_port = &event["src_port"]
                .as_u64()
                .ok_or(Error::InvalidDestinationPort)?;
            let dest_port = &event["dest_port"]
                .as_u64()
                .ok_or(Error::InvalidSourcePort)?;
            match (src_ip, dest_ip) {
                (IpAddr::V4(src), IpAddr::V4(dst)) => {
                    let udp = packet::UdpBuilder::new(*src_port as u16, *dest_port as u16)
                        .payload(payload)
                        .build();
                    let packet = packet::Ip4Builder::new()
                        .source(src)
                        .destination(dst)
                        .protocol(proto)
                        .payload(udp)
                        .build();
                    return Ok(packet);
                }
                (IpAddr::V6(_src), IpAddr::V6(_dst)) => {
                    return Err(Error::Ipv6NotSupported);
                }
                _ => {
                    return Err(Error::MissMatchedIpAddrVersions);
                }
            }
        }
        _ => {
            return Err(Error::ProtocolNotSupported(format!("{:?}", proto)));
        }
    }
}

pub fn create(linktype: u32, ts: time::OffsetDateTime, packet: &[u8]) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(FILE_HEADER_LEN + PACKET_HEADER_LEN + packet.len());

    // Write out the file header.
    buf.put_u32_le(MAGIC);
    buf.put_u16_le(VERSION_MAJOR);
    buf.put_u16_le(VERSION_MINOR);
    buf.put_u32_le(0); // This zone (GMT to local correction)
    buf.put_u32_le(0); // Accuracy of timestamps (sigfigs)
    buf.put_u32_le(0); // Snap length
    buf.put_u32_le(linktype); // Data link type

    // The record header.
    buf.put_u32_le(ts.unix_timestamp_nanos() as u32);
    buf.put_u32_le(ts.microsecond());
    buf.put_u32_le(packet.len() as u32);
    buf.put_u32_le(packet.len() as u32);
    buf.put_slice(packet);

    buf.to_vec()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::eve::Eve;

    #[test]
    fn test_pcap_file() {
        let linktype = LinkType::Ethernet;
        let eve_timestamp = "2020-05-01T08:50:23.297919-0600";
        let packet_base64 =
            "oDafTEwo2MuK7aFGCABFAAAobXBAAEAGMBcKEAELzE/F3qMmAbtL2EhIK8jWtFAQAenVIwAAAAAAAAAA";
        let ts = crate::eve::parse_eve_timestamp(eve_timestamp).unwrap();
        let packet = base64::decode(packet_base64).unwrap();
        let _pcap_buffer = super::create(linktype as u32, ts, &packet);
    }

    #[test]
    fn test_packet_from_payload() {
        let event: EveJson = serde_json::from_str(TEST_EVE_RECORD).unwrap();
        let packet = super::packet_from_payload(&event).unwrap();
        let ts = event.timestamp().unwrap();
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
    "packet_info": {
      "linktype": 1
    },
    "payload": "fGdvbGFudHVzLmNvbSI7IGRpc3RhbmNlOjE7IHdpdGhpbjoxMzsgcmVmZXJlbmNlOnVybCxzc2xibC5hYnVzZS5jaDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTE7IHJldjoyOyBtZXRhZGF0YTphdHRhY2tfdGFyZ2V0IENsaWVudF9FbmRwb2ludCwgZGVwbG95bWVudCBQZXJpbWV0ZXIsIHRhZyBTU0xfTWFsaWNpb3VzX0NlcnQsIHNpZ25hdHVyZV9zZXZlcml0eSBNYWpvciwgY3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTZfMDdfMDE7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAxIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfFByaXZ8MjB8VmVyc2lvbiI7IGZhc3RfcGF0dGVybjsgZGlzdGFuY2U6MDsgcmVmZXJlbmNlOnVybCxwYXN0ZWJpbi5jb20vRUgxU0g5YUw7IGNsYXNzdHlwZTp0cm9qYW4tYWN0aXZpdHk7IHNpZDoyMDIxOTEyOyByZXY6MTsgbWV0YWRhdGE6Y3JlYXRlZF9hdCAyMDE1XzEwXzA2LCB1cGRhdGVkX2F0IDIwMTVfMTBfMDY7KQphbGVydCB0Y3AgYW55IGFueSAtPiBhbnkgYW55IChtc2c6IkVUIE1BTFdBUkUgRUxGL211Qm9UIElSQyBBY3Rpdml0eSAyIjsgZmxvdzplc3RhYmxpc2hlZCxmcm9tX3NlcnZlcjsgY29udGVudDoiTk9USUNFIjsgY29udGVudDoifDNhfG11Qm9UfDIwfHNheXN8MjB8IjsgZmFzdF9wYXR0ZXJuOyBkaXN0YW5jZTowOyByZWZlcmVuY2U6dXJsLHBhc3RlYmluLmNvbS9FSDFTSDlhTDsgY2xhc3N0eXBlOnRyb2phbi1hY3Rpdml0eTsgc2lkOjIwMjE5MTM7IHJldjoxOyBtZXRhZGF0YTpjcmVhdGVkX2F0IDIwMTVfMTBfMDYsIHVwZGF0ZWRfYXQgMjAxNV8xMF8wNjspCmFsZXJ0IHRjcCBhbnkgYW55IC0+IGFueSBhbnkgKG1zZzoiRVQgTUFMV0FSRSBFTEYvbXVCb1QgSVJDIEFjdGl2aXR5IDMiOyBmbG93OmVzdGFibGlzaGVkLGZyb21fc2VydmVyOyBjb250ZW50OiJOT1RJQ0UiOyBjb250ZW50OiJ8M2F8W0FwYWNoZSAvIFBIUCA1LngiOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGViaW4uY29tL0VIMVNIOWFMOyBjbGFzc3R5cGU6dHJvamFuLWFjdGl2aXR5OyBzaWQ6MjAyMTkxNDsgcmV2OjE7IG1ldGFkYXRhOmNyZWF0ZWRfYXQgMjAxNV8xMF8wNiwgdXBkYXRlZF9hdCAyMDE1XzEwXzA2OykKYWxlcnQgdGNwIGFueSBhbnkgLT4gYW55IGFueSAobXNnOiJFVCBNQUxXQVJFIEVMRi9tdUJvVCBJUkMgQWN0aXZpdHkgNCI7IGZsb3c6ZXN0YWJsaXNoZWQsZnJvbV9zZXJ2ZXI7IGNvbnRlbnQ6Ik5PVElDRSI7IGNvbnRlbnQ6IkZMT09EIDx0YXJnZXQ+IDxwb3J0PiA8c2Vjcz4iOyBmYXN0X3BhdHRlcm47IGRpc3RhbmNlOjA7IHJlZmVyZW5jZTp1cmwscGFzdGU=",
    "payload_printable": "|golantus.com\"; distance:1; within:13; reference:url,sslbl.abuse.ch; classtype:trojan-activity; sid:2021911; rev:2; metadata:attack_target Client_Endpoint, deployment Perimeter, tag SSL_Malicious_Cert, signature_severity Major, created_at 2015_10_06, updated_at 2016_07_01;)\nalert tcp any any -> any any (msg:\"ET MALWARE ELF/muBoT IRC Activity 1\"; flow:established,from_server; content:\"NOTICE\"; content:\"|3a|muBoT|20|Priv|20|Version\"; fast_pattern; distance:0; reference:url,pastebin.com/EH1SH9aL; classtype:trojan-activity; sid:2021912; rev:1; metadata:created_at 2015_10_06, updated_at 2015_10_06;)\nalert tcp any any -> any any (msg:\"ET MALWARE ELF/muBoT IRC Activity 2\"; flow:established,from_server; content:\"NOTICE\"; content:\"|3a|muBoT|20|says|20|\"; fast_pattern; distance:0; reference:url,pastebin.com/EH1SH9aL; classtype:trojan-activity; sid:2021913; rev:1; metadata:created_at 2015_10_06, updated_at 2015_10_06;)\nalert tcp any any -> any any (msg:\"ET MALWARE ELF/muBoT IRC Activity 3\"; flow:established,from_server; content:\"NOTICE\"; content:\"|3a|[Apache / PHP 5.x\"; fast_pattern; distance:0; reference:url,pastebin.com/EH1SH9aL; classtype:trojan-activity; sid:2021914; rev:1; metadata:created_at 2015_10_06, updated_at 2015_10_06;)\nalert tcp any any -> any any (msg:\"ET MALWARE ELF/muBoT IRC Activity 4\"; flow:established,from_server; content:\"NOTICE\"; content:\"FLOOD <target> <port> <secs>\"; fast_pattern; distance:0; reference:url,paste",
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
