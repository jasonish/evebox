// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Shared test helpers for building PCAP spool fixtures.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

/// A minimal Ethernet/IPv4/UDP packet from 192.0.2.1:1234 to
/// 198.51.100.2 and the given destination port.
pub(crate) fn udp_packet(destination_port: u16) -> Vec<u8> {
    let mut packet = vec![0; 14 + 20 + 8];
    packet[12..14].copy_from_slice(&[0x08, 0x00]);
    packet[14] = 0x45;
    packet[16..18].copy_from_slice(&28_u16.to_be_bytes());
    packet[22] = 64;
    packet[23] = 17;
    packet[26..30].copy_from_slice(&[192, 0, 2, 1]);
    packet[30..34].copy_from_slice(&[198, 51, 100, 2]);
    packet[34..36].copy_from_slice(&1234_u16.to_be_bytes());
    packet[36..38].copy_from_slice(&destination_port.to_be_bytes());
    packet[38..40].copy_from_slice(&8_u16.to_be_bytes());
    packet
}

/// Write an Ethernet pcap file of UDP packets given as (timestamp-seconds,
/// destination-port) pairs.
pub(crate) fn write_pcap_file(path: &Path, packets: &[(i64, u16)]) {
    write_pcap_file_with_linktype(path, packets, pcap::Linktype::ETHERNET);
}

pub(crate) fn write_pcap_file_with_linktype(
    path: &Path,
    packets: &[(i64, u16)],
    linktype: pcap::Linktype,
) {
    let dead = pcap::Capture::dead(linktype).unwrap();
    let mut savefile = dead.savefile(path).unwrap();
    for (timestamp, destination_port) in packets {
        let packet_data = udp_packet(*destination_port);
        let header = pcap::PacketHeader {
            ts: libc::timeval {
                tv_sec: *timestamp,
                tv_usec: 0,
            },
            caplen: packet_data.len() as u32,
            len: packet_data.len() as u32,
        };
        savefile.write(&pcap::Packet::new(&header, &packet_data));
    }
}

/// An Ethernet frame with zeroed MAC addresses and the given
/// ethertype, ready for a payload.
fn ethernet(ethertype: u16) -> Vec<u8> {
    let mut frame = vec![0; 14];
    frame[12..14].copy_from_slice(&ethertype.to_be_bytes());
    frame
}

/// An 8 byte generic layer 4 header carrying just the ports, layed
/// out as TCP, UDP and SCTP all do: source port at offset 0,
/// destination port at offset 2.
pub(crate) fn ports(source_port: u16, destination_port: u16) -> Vec<u8> {
    let mut header = vec![0; 8];
    header[0..2].copy_from_slice(&source_port.to_be_bytes());
    header[2..4].copy_from_slice(&destination_port.to_be_bytes());
    header
}

/// An Ethernet/IPv4 packet with the given protocol and payload.
pub(crate) fn ipv4_packet(proto: u8, src: &str, dst: &str, payload: &[u8]) -> Vec<u8> {
    ipv4_fragment(proto, src, dst, 0, payload)
}

/// An IPv4 datagram with no link layer header, for raw-IP
/// (LINKTYPE_RAW) captures.
pub(crate) fn ipv4_datagram(proto: u8, src: &str, dst: &str, payload: &[u8]) -> Vec<u8> {
    let src: Ipv4Addr = src.parse().unwrap();
    let dst: Ipv4Addr = dst.parse().unwrap();
    let mut packet = vec![0; 20];
    packet[0] = 0x45;
    packet[2..4].copy_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
    packet[8] = 64;
    packet[9] = proto;
    packet[12..16].copy_from_slice(&src.octets());
    packet[16..20].copy_from_slice(&dst.octets());
    packet.extend_from_slice(payload);
    packet
}

/// Like [`ipv4_packet`] but with the raw IPv4 flags/fragment-offset
/// field set: 0x2000 is more-fragments with offset 0, any value with
/// the low 13 bits set is a fragment continuation.
pub(crate) fn ipv4_fragment(
    proto: u8,
    src: &str,
    dst: &str,
    frag_field: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut packet = ethernet(0x0800);
    let mut datagram = ipv4_datagram(proto, src, dst, payload);
    datagram[6..8].copy_from_slice(&frag_field.to_be_bytes());
    packet.extend_from_slice(&datagram);
    packet
}

/// An Ethernet/IPv6 packet with the given next header and payload.
pub(crate) fn ipv6_packet(proto: u8, src: &str, dst: &str, payload: &[u8]) -> Vec<u8> {
    let src: Ipv6Addr = src.parse().unwrap();
    let dst: Ipv6Addr = dst.parse().unwrap();
    let mut packet = ethernet(0x86dd);
    let mut header = vec![0; 40];
    header[0] = 0x60;
    header[4..6].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    header[6] = proto;
    header[7] = 64;
    header[8..24].copy_from_slice(&src.octets());
    header[24..40].copy_from_slice(&dst.octets());
    packet.extend_from_slice(&header);
    packet.extend_from_slice(payload);
    packet
}

/// Insert an 802.1Q VLAN tag into an Ethernet frame.
pub(crate) fn vlan_tag(frame: &[u8], vlan_id: u16) -> Vec<u8> {
    let mut tagged = Vec::with_capacity(frame.len() + 4);
    tagged.extend_from_slice(&frame[..12]);
    tagged.extend_from_slice(&0x8100_u16.to_be_bytes());
    tagged.extend_from_slice(&(vlan_id & 0x0fff).to_be_bytes());
    tagged.extend_from_slice(&frame[12..]);
    tagged
}

/// Write an Ethernet pcap file of pre-built frames, one second apart
/// starting at 1700000000.
pub(crate) fn write_raw_pcap_file(path: &Path, packets: &[Vec<u8>]) {
    let dead = pcap::Capture::dead(pcap::Linktype::ETHERNET).unwrap();
    let mut savefile = dead.savefile(path).unwrap();
    for (i, packet_data) in packets.iter().enumerate() {
        let header = pcap::PacketHeader {
            ts: libc::timeval {
                tv_sec: 1_700_000_000 + i as i64,
                tv_usec: 0,
            },
            caplen: packet_data.len() as u32,
            len: packet_data.len() as u32,
        };
        savefile.write(&pcap::Packet::new(&header, packet_data));
    }
}

/// The on-disk link type value of raw-IP captures (LINKTYPE_RAW),
/// as written by libpcap for tun and similar interfaces.
pub(crate) const LINKTYPE_RAW: u32 = 101;

/// Write a classic pcap file with the given on-disk link type value
/// and pre-built packets, one second apart starting at 1700000000.
/// Written directly rather than through libpcap so the on-disk link
/// type value is exact (libpcap remaps between DLT_* and LINKTYPE_*
/// values on write).
pub(crate) fn write_pcap_file_bytes(path: &Path, linktype: u32, packets: &[Vec<u8>]) {
    use crate::util::pcap::{create_header_with_snaplen, create_record_raw};
    let mut out = create_header_with_snaplen(linktype, 65_535);
    for (i, packet_data) in packets.iter().enumerate() {
        out.extend_from_slice(&create_record_raw(
            (1_700_000_000 + i) as u32,
            0,
            packet_data.len() as u32,
            packet_data,
        ));
    }
    std::fs::write(path, out).unwrap();
}

pub(crate) fn count_packets(path: &Path) -> usize {
    let mut capture = pcap::Capture::from_file(path).unwrap();
    let mut count = 0;
    while capture.next_packet().is_ok() {
        count += 1;
    }
    count
}
