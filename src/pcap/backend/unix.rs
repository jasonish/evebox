// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub(crate) use pcap::{BpfProgram, Capture, Error, Linktype, Offline, Packet, PacketHeader};

pub(crate) fn ensure_available() -> Result<(), Error> {
    Ok(())
}

/// The public `pcap::BpfProgram::filter` helper synthesizes a packet
/// header with `len == caplen`. Use libpcap's offline evaluator with
/// the real packet header so free-form filters using `len` retain
/// their normal libpcap semantics for snaplen-truncated packets.
#[repr(C)]
struct RawBpfProgram {
    bf_len: libc::c_uint,
    bf_insns: *const pcap::BpfInstruction,
}

unsafe extern "C" {
    #[link_name = "pcap_offline_filter"]
    fn offline_filter(
        program: *const RawBpfProgram,
        header: *const pcap::PacketHeader,
        data: *const libc::c_uchar,
    ) -> libc::c_int;
}

pub(crate) fn bpf_matches(program: &pcap::BpfProgram, packet: &pcap::Packet<'_>) -> bool {
    let instructions = program.get_instructions();
    let raw = RawBpfProgram {
        bf_len: instructions.len() as libc::c_uint,
        bf_insns: instructions.as_ptr(),
    };
    // SAFETY: `raw` points at the instructions owned by `program`, and
    // the packet header and data remain valid for the duration of the
    // call. Both structures have the C layouts expected by libpcap.
    unsafe { offline_filter(&raw, packet.header, packet.data.as_ptr()) > 0 }
}
