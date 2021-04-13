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

//! Just enough packet building to convert Eve payload's into a packet.
//!
//! TODO: Ipv6

use bytes::{Buf, BufMut, BytesMut};
use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub enum Protocol {
    Tcp,
    Udp,
    Other(u8),
}

impl Protocol {
    pub fn from_name(proto: &str) -> Option<Protocol> {
        let proto = proto.to_lowercase();
        match proto.as_str() {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

impl Into<u8> for Protocol {
    fn into(self) -> u8 {
        match self {
            Self::Tcp => 6,
            Self::Udp => 17,
            Self::Other(n) => n,
        }
    }
}

pub struct UdpBuilder {
    source_port: u16,
    destination_port: u16,
    payload: Option<Vec<u8>>,
}

impl UdpBuilder {
    pub fn new(source_port: u16, destination_port: u16) -> Self {
        Self {
            source_port,
            destination_port,
            payload: None,
        }
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn build(self) -> Vec<u8> {
        let payload = self.payload.unwrap_or_default();
        let mut buf = BytesMut::new();
        buf.put_u16(self.source_port);
        buf.put_u16(self.destination_port);
        buf.put_u16(8 + payload.len() as u16);
        buf.put_u16(0);
        buf.extend_from_slice(&payload);
        let udp_buf = buf.to_vec();
        return udp_buf;
    }
}

pub struct TcpBuilder {
    source_port: u16,
    destination_port: u16,
    payload: Vec<u8>,
}

impl TcpBuilder {
    pub fn new(source_port: u16, destination_port: u16) -> Self {
        Self {
            source_port,
            destination_port,
            payload: vec![],
        }
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn build(self) -> Vec<u8> {
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

        return buf.to_vec();
    }
}

pub struct Ip4Builder {
    source_addr: Ipv4Addr,
    dest_addr: Ipv4Addr,
    protocol: Protocol,
    ttl: u8,
    payload: Vec<u8>,
}

impl Ip4Builder {
    pub fn new() -> Self {
        Self {
            source_addr: Ipv4Addr::new(0, 0, 0, 0),
            dest_addr: Ipv4Addr::new(0, 0, 0, 0),
            ttl: 255,
            protocol: Protocol::Udp,
            payload: vec![],
        }
    }

    pub fn source(mut self, addr: Ipv4Addr) -> Self {
        self.source_addr = addr;
        self
    }

    pub fn destination(mut self, addr: Ipv4Addr) -> Self {
        self.dest_addr = addr;
        self
    }

    pub fn ttl(mut self, ttl: u8) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn build(self) -> Vec<u8> {
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
        out[10] = (csum >> 8 & 0xffu16) as u8;
        out[11] = (csum & 0xff) as u8;

        if let Protocol::Udp = &self.protocol {
            let csum = &self.tcpudp_checksum(&out, Protocol::Udp);
            out[26] = (csum >> 8 & 0xffu16) as u8;
            out[27] = (csum & 0xff) as u8;
        } else if let Protocol::Tcp = &self.protocol {
            let csum = &self.tcpudp_checksum(&out, Protocol::Tcp);
            out[36] = (csum >> 8 & 0xffu16) as u8;
            out[37] = (csum & 0xff) as u8;
        }

        return out;
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
        return !result as u16;
    }

    fn tcpudp_checksum(&self, input: &[u8], proto: Protocol) -> u16 {
        let mut result = 0xffffu32;
        let mut pseudo = BytesMut::new();
        pseudo.extend_from_slice(&input[12..20]);
        pseudo.put_u8(0);
        pseudo.put_u8(proto.into());
        pseudo.put_u16(input.len() as u16 - 20 as u16);

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

        return !result as u16;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pcap;

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
        let now = chrono::Utc::now();
        let _pcap_buffer = pcap::create(pcap::LinkType::Raw as u32, now, &packet);
    }
}
