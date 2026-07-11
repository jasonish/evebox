// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Streaming classic-pcap writer with a lazily written file header.

use std::io::Write;

use crate::util::pcap::{
    FILE_HEADER_LEN, PCAP_RECORD_HEADER_SIZE, create_header, create_record_raw,
};

/// Writes packets as a classic little-endian pcap stream. Nothing is
/// written until the first packet, so an output with zero bytes written
/// means no packet matched.
pub(crate) struct PcapWriter<'a> {
    out: &'a mut dyn Write,
    bytes: u64,
    header_written: bool,
}

impl<'a> PcapWriter<'a> {
    pub(crate) fn new(out: &'a mut dyn Write) -> Self {
        Self {
            out,
            bytes: 0,
            header_written: false,
        }
    }

    /// Total bytes written so far, including the pcap file header.
    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes
    }

    /// The number of bytes that writing a packet with `data_len` bytes of
    /// packet data would add to the output, including the lazily written
    /// file header if it has not been written yet.
    pub(crate) fn next_write_size(&self, data_len: usize) -> u64 {
        let mut size = (PCAP_RECORD_HEADER_SIZE + data_len) as u64;
        if !self.header_written {
            size += FILE_HEADER_LEN as u64;
        }
        size
    }

    /// Write a single packet record, writing the file header first if this
    /// is the first packet. The captured length is the packet data actually
    /// available, the original length comes from the packet header.
    pub(crate) fn write_packet(
        &mut self,
        linktype: pcap::Linktype,
        pkt: &pcap::Packet,
    ) -> std::io::Result<()> {
        if !self.header_written {
            // The first packet cannot predict the largest later caplen, so
            // advertise the classic-PCAP maximum rather than a snaplen that
            // a later emitted record could exceed.
            let header = create_header(linktype.0 as u32);
            self.out.write_all(&header)?;
            self.bytes += header.len() as u64;
            self.header_written = true;
        }
        let record = create_record_raw(
            pkt.header.ts.tv_sec as u32,
            pkt.header.ts.tv_usec as u32,
            pkt.header.len,
            pkt.data,
        );
        self.out.write_all(&record)?;
        self.bytes += record.len() as u64;
        Ok(())
    }

    /// Flush the inner writer. A no-op stream-wise before the first
    /// packet: nothing has been written, so nothing is pushed down.
    pub(crate) fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_lazy_header_and_byte_accounting() {
        let mut buf = vec![];
        let mut writer = PcapWriter::new(&mut buf);
        assert_eq!(writer.bytes_written(), 0);
        assert_eq!(
            writer.next_write_size(42),
            (FILE_HEADER_LEN + PCAP_RECORD_HEADER_SIZE + 42) as u64
        );

        let data = [0u8; 42];
        let header = pcap::PacketHeader {
            ts: libc::timeval {
                tv_sec: 1_700_000_000,
                tv_usec: 123,
            },
            caplen: 42,
            len: 60,
        };
        writer
            .write_packet(pcap::Linktype::ETHERNET, &pcap::Packet::new(&header, &data))
            .unwrap();
        assert_eq!(
            writer.bytes_written(),
            (FILE_HEADER_LEN + PCAP_RECORD_HEADER_SIZE + 42) as u64
        );

        // A second record does not repeat the file header.
        assert_eq!(
            writer.next_write_size(42),
            (PCAP_RECORD_HEADER_SIZE + 42) as u64
        );
        writer
            .write_packet(pcap::Linktype::ETHERNET, &pcap::Packet::new(&header, &data))
            .unwrap();
        let total = FILE_HEADER_LEN + 2 * (PCAP_RECORD_HEADER_SIZE + 42);
        assert_eq!(writer.bytes_written(), total as u64);
        assert_eq!(buf.len(), total);

        // File header fields: LE magic, maximum snaplen, linktype 1.
        assert_eq!(&buf[0..4], &0xa1b2_c3d4u32.to_le_bytes());
        assert_eq!(&buf[16..20], &u32::MAX.to_le_bytes());
        assert_eq!(&buf[20..24], &1u32.to_le_bytes());

        // First record header: ts_sec, ts_usec, incl_len (data length),
        // orig_len (from the packet header).
        assert_eq!(&buf[24..28], &1_700_000_000u32.to_le_bytes());
        assert_eq!(&buf[28..32], &123u32.to_le_bytes());
        assert_eq!(&buf[32..36], &42u32.to_le_bytes());
        assert_eq!(&buf[36..40], &60u32.to_le_bytes());
    }

    #[test]
    fn test_global_snaplen_covers_oversized_record() {
        let mut buf = vec![];
        let mut writer = PcapWriter::new(&mut buf);
        let data = vec![0u8; 70_000];
        let header = pcap::PacketHeader {
            ts: libc::timeval {
                tv_sec: 1_700_000_000,
                tv_usec: 0,
            },
            caplen: data.len() as u32,
            len: data.len() as u32,
        };

        writer
            .write_packet(pcap::Linktype::ETHERNET, &pcap::Packet::new(&header, &data))
            .unwrap();

        let snaplen = u32::from_le_bytes(buf[16..20].try_into().unwrap());
        let incl_len = u32::from_le_bytes(buf[32..36].try_into().unwrap());
        assert!(snaplen >= incl_len);
        assert_eq!(incl_len, 70_000);
    }
}
