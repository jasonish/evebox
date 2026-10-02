// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Offline-only Npcap bindings. No import library or capture driver is needed
//! at build time. Only the administrator-installed System32/Npcap/wpcap.dll
//! is loaded; neither PATH, the working directory, nor legacy WinPcap is tried.
//!
//! Initialize UTF-8 string handling once, before any captures are opened.
//! This is EveBox's only libpcap consumer on Windows, so it owns the library's
//! process-global encoding choice. Require pcap_init (libpcap 1.10+) rather
//! than silently interpreting UTF-8 paths in the local ANSI code page.
//! In particular, do not pass a Rust CRT FILE* into Npcap's CRT.
//! See https://npcap.com/guide/wpcap/pcap_init.html and
//! https://npcap.com/guide/wpcap/pcap_open_offline.html.

use std::ffi::{CStr, CString, OsString, c_char, c_int, c_uint};
use std::marker::PhantomData;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::sync::OnceLock;

use libloading::os::windows::Library;
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

const ERRBUF_SIZE: usize = 256;
const PCAP_CHAR_ENC_UTF_8: c_uint = 1;
const LOAD_FLAGS: u32 = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no more packets")]
    NoMorePackets,
    #[error("{0}")]
    PcapError(String),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Linktype(pub(crate) i32);

impl Linktype {
    pub(crate) const ETHERNET: Self = Self(1);
}

// Npcap uses Winsock timeval (two 32-bit C longs), even on 64-bit Windows.
// Do not replace these with time_t, Rust i64, or Unix's timeval layout.
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct PacketHeader {
    pub(crate) ts: libc::timeval,
    pub(crate) caplen: u32,
    pub(crate) len: u32,
}

pub(crate) struct Packet<'a> {
    pub(crate) header: &'a PacketHeader,
    pub(crate) data: &'a [u8],
}

impl<'a> Packet<'a> {
    pub(crate) fn new(header: &'a PacketHeader, data: &'a [u8]) -> Self {
        Self { header, data }
    }
}

#[repr(C)]
struct Pcap {
    _private: [u8; 0],
}

// Layouts from Npcap's pcap/bpf.h: bpf_u_int32 is unsigned int, not
// a pointer-sized integer. The only pointer-sized member is bf_insns.
#[repr(C)]
struct BpfInsn {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct RawBpfProgram {
    bf_len: c_uint,
    bf_insns: *mut BpfInsn,
}

// Check the most consequential ABI assumptions even outside test builds.
const _: () = {
    assert!(size_of::<libc::c_long>() == 4);
    assert!(size_of::<libc::timeval>() == 8);
    assert!(size_of::<PacketHeader>() == 16);
    assert!(size_of::<BpfInsn>() == 8);
};

// Npcap exports cdecl, NOT Windows' stdcall (extern "system") on x86.
// These declarations match pcap/pcap.h; no live-capture API is resolved.
struct Api {
    init: unsafe extern "C" fn(c_uint, *mut c_char) -> c_int,
    open_offline: unsafe extern "C" fn(*const c_char, *mut c_char) -> *mut Pcap,
    open_dead: unsafe extern "C" fn(c_int, c_int) -> *mut Pcap,
    close: unsafe extern "C" fn(*mut Pcap),
    datalink: unsafe extern "C" fn(*mut Pcap) -> c_int,
    next_ex: unsafe extern "C" fn(*mut Pcap, *mut *mut PacketHeader, *mut *const u8) -> c_int,
    geterr: unsafe extern "C" fn(*mut Pcap) -> *mut c_char,
    compile:
        unsafe extern "C" fn(*mut Pcap, *mut RawBpfProgram, *const c_char, c_int, u32) -> c_int,
    freecode: unsafe extern "C" fn(*mut RawBpfProgram),
    offline_filter:
        unsafe extern "C" fn(*const RawBpfProgram, *const PacketHeader, *const u8) -> c_uint,
    // Own the library for at least as long as its function pointers. The
    // process-global OnceLock is never dropped, so successful loads stay live.
    _library: Library,
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

fn api() -> Result<&'static Api, Error> {
    API.get_or_init(Api::load)
        .as_ref()
        .map_err(|message| Error::PcapError(message.clone()))
}

pub(crate) fn ensure_available() -> Result<(), Error> {
    api().map(|_| ())
}

fn system_directory() -> Result<PathBuf, String> {
    let mut buffer = vec![0u16; 260];
    loop {
        // SAFETY: buffer contains writable u16 elements and its length fits
        // in u32 (initially 260, subsequently a length returned by Windows).
        let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 {
            return Err(format!(
                "cannot locate the Windows system directory: {}",
                std::io::Error::last_os_error()
            ));
        }
        if (length as usize) < buffer.len() {
            let path = PathBuf::from(OsString::from_wide(&buffer[..length as usize]));
            if !path.is_absolute() {
                return Err("Windows returned a non-absolute system directory".into());
            }
            return Ok(path);
        }
        // For an undersized buffer, Windows returns the required size
        // INCLUDING the NUL; success excludes it.
        buffer.resize(length as usize, 0);
    }
}

impl Api {
    fn load() -> Result<Self, String> {
        let path = system_directory()?.join("Npcap").join("wpcap.dll");
        Self::load_from(&path)
    }

    // Private loader seam for tests. Production only supplies the path above;
    // this must never be exposed as a user-selected library path.
    fn load_from(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("Npcap library path must be absolute".into());
        }
        // SAFETY: production loads only the administrator-controlled Npcap
        // installation. Its DLL initialization must be trusted like any
        // native dependency. LoadLibraryExW searches dependencies only in the
        // DLL's directory and System32; no process-global search path changes.
        let library = unsafe { Library::load_with_flags(path, LOAD_FLAGS) }.map_err(|error| {
            format!(
                "Npcap is unavailable ({}): {error}; install Npcap and restart EveBox",
                path.display()
            )
        })?;
        let api = Self::resolve(library)
            .map_err(|error| format!("incompatible Npcap library ({}): {error}", path.display()))?;
        let mut errbuf = [0; ERRBUF_SIZE];
        // SAFETY: the fixed encoding constant is valid and errbuf has
        // PCAP_ERRBUF_SIZE writable bytes. Production calls this only from
        // OnceLock initialization, before exposing any API function pointers.
        if unsafe { (api.init)(PCAP_CHAR_ENC_UTF_8, errbuf.as_mut_ptr()) } != 0 {
            return Err(format!(
                "Npcap initialization failed: {}",
                error_buffer(&errbuf)
            ));
        }
        Ok(api)
    }

    fn resolve(library: Library) -> Result<Self, libloading::Error> {
        // SAFETY: the fixed symbols and field types match Npcap's C ABI.
        // Copying the pointers is safe because this Api owns the Library.
        // On a missing symbol, Library is dropped and no pointers escape.
        unsafe {
            Ok(Self {
                init: *library.get(b"pcap_init\0")?,
                open_offline: *library.get(b"pcap_open_offline\0")?,
                open_dead: *library.get(b"pcap_open_dead\0")?,
                close: *library.get(b"pcap_close\0")?,
                datalink: *library.get(b"pcap_datalink\0")?,
                next_ex: *library.get(b"pcap_next_ex\0")?,
                geterr: *library.get(b"pcap_geterr\0")?,
                compile: *library.get(b"pcap_compile\0")?,
                freecode: *library.get(b"pcap_freecode\0")?,
                offline_filter: *library.get(b"pcap_offline_filter\0")?,
                _library: library,
            })
        }
    }
}

fn filename(path: &Path) -> Result<CString, Error> {
    let name = path
        .to_str()
        .ok_or_else(|| Error::PcapError("PCAP path is not valid Unicode".into()))?;
    // pcap_open_offline treats '-' as stdin, not a filename.
    if name == "-" {
        return Err(Error::PcapError(
            "Npcap offline capture does not support standard input".into(),
        ));
    }
    CString::new(name).map_err(|_| Error::PcapError("PCAP path contains a NUL byte".into()))
}

fn error_buffer(buffer: &[c_char; ERRBUF_SIZE]) -> String {
    let bytes: Vec<_> = buffer
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(crate) enum Offline {}
pub(crate) enum Dead {}

// NonNull deliberately keeps captures !Send and !Sync. In particular,
// compile(&self) calls C code which can mutate the capture's error buffer.
// The Api itself contains only immutable function pointers and Library.
pub(crate) struct Capture<T> {
    handle: NonNull<Pcap>,
    api: &'static Api,
    _state: PhantomData<T>,
}

impl Capture<Offline> {
    pub(crate) fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let filename = filename(path.as_ref())?;
        let api = api()?;
        let mut errbuf = [0; ERRBUF_SIZE];
        // SAFETY: filename is NUL terminated and errbuf has PCAP_ERRBUF_SIZE
        // writable bytes. On success we exclusively own the returned handle.
        let handle = unsafe { (api.open_offline)(filename.as_ptr(), errbuf.as_mut_ptr()) };
        let handle = NonNull::new(handle)
            .ok_or_else(|| Error::PcapError(format!("opening PCAP: {}", error_buffer(&errbuf))))?;
        Ok(Self {
            handle,
            api,
            _state: PhantomData,
        })
    }

    pub(crate) fn next_packet(&mut self) -> Result<Packet<'_>, Error> {
        let mut header = ptr::null_mut();
        let mut data = ptr::null();
        // SAFETY: handle is live and exclusively borrowed; the out-pointers
        // are writable. Npcap owns the returned buffers until the next read
        // or close, both prevented while the returned Packet is borrowed.
        let status = unsafe { (self.api.next_ex)(self.handle.as_ptr(), &mut header, &mut data) };
        match status {
            1 => {
                // SAFETY: a successful next_ex returns a valid, aligned
                // pcap_pkthdr pointer. Check NULL before dereferencing.
                let header = unsafe { header.as_ref() }.ok_or_else(|| {
                    Error::PcapError("Npcap returned a null packet header".into())
                })?;
                let length = header.caplen as usize;
                if length > isize::MAX as usize || (length != 0 && data.is_null()) {
                    return Err(Error::PcapError(
                        "Npcap returned invalid packet data".into(),
                    ));
                }
                let data = if length == 0 {
                    &[]
                } else {
                    // SAFETY: Npcap guarantees caplen initialized bytes on
                    // success. The bound above satisfies Rust's slice size
                    // requirement also on 32-bit Windows. The mutable borrow
                    // of self prevents invalidating this slice.
                    unsafe { std::slice::from_raw_parts(data, length) }
                };
                Ok(Packet::new(header, data))
            }
            -2 => Err(Error::NoMorePackets),
            -1 => Err(self.error()),
            // Offline reads cannot time out. Do not silently treat an
            // unexpected return value as EOF or spin indefinitely on it.
            other => Err(Error::PcapError(format!(
                "unexpected Npcap offline read status: {other}"
            ))),
        }
    }
}

impl Capture<Dead> {
    pub(crate) fn dead(linktype: Linktype) -> Result<Self, Error> {
        let api = api()?;
        // SAFETY: both arguments are integers with no pointer obligations;
        // the returned handle, if non-NULL, is exclusively owned here.
        let handle = unsafe { (api.open_dead)(linktype.0, 65535) };
        let handle = NonNull::new(handle)
            .ok_or_else(|| Error::PcapError("Npcap could not create a dead capture".into()))?;
        Ok(Self {
            handle,
            api,
            _state: PhantomData,
        })
    }
}

impl<T> Capture<T> {
    pub(crate) fn get_datalink(&self) -> Linktype {
        // SAFETY: this capture owns a live handle and cannot be shared across
        // threads. Datalink is available for both dead and offline captures.
        Linktype(unsafe { (self.api.datalink)(self.handle.as_ptr()) })
    }

    pub(crate) fn compile(&self, expression: &str, optimize: bool) -> Result<BpfProgram, Error> {
        let expression = CString::new(expression)
            .map_err(|_| Error::PcapError("BPF expression contains a NUL byte".into()))?;
        let mut raw = RawBpfProgram {
            bf_len: 0,
            bf_insns: ptr::null_mut(),
        };
        // SAFETY: handle is live, raw is writable, and expression is NUL
        // terminated. PCAP_NETMASK_UNKNOWN is 0xffffffff. On success Npcap
        // transfers the compiled instructions to us; on failure it cleans
        // its compilation allocations itself.
        let status = unsafe {
            (self.api.compile)(
                self.handle.as_ptr(),
                &mut raw,
                expression.as_ptr(),
                c_int::from(optimize),
                u32::MAX,
            )
        };
        if status != 0 {
            return Err(self.error());
        }
        Ok(BpfProgram { raw, api: self.api })
    }

    fn error(&self) -> Error {
        // SAFETY: pcap_geterr returns a capture-owned NUL-terminated string,
        // copied before any subsequent operation on this unshared handle.
        let message = unsafe { (self.api.geterr)(self.handle.as_ptr()) };
        let message = if message.is_null() {
            "unspecified Npcap error".into()
        } else {
            // SAFETY: the non-NULL string remains valid while self is live.
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        Error::PcapError(message)
    }
}

impl<T> Drop for Capture<T> {
    fn drop(&mut self) {
        // SAFETY: we own exactly one live handle, and no packet borrow can
        // survive its capture. The process-global Api keeps the DLL loaded.
        unsafe { (self.api.close)(self.handle.as_ptr()) }
    }
}

pub(crate) struct BpfProgram {
    raw: RawBpfProgram,
    api: &'static Api,
}

impl Drop for BpfProgram {
    fn drop(&mut self) {
        // SAFETY: raw came from successful pcap_compile and has exactly one
        // owner. Free with the same DLL/CRT that allocated the instructions.
        unsafe { (self.api.freecode)(&mut self.raw) }
    }
}

pub(crate) fn bpf_matches(program: &BpfProgram, packet: &Packet<'_>) -> bool {
    // Packet::new and the public fields allow callers to supply inconsistent
    // metadata. Never let the C interpreter read past the Rust slice.
    if packet.header.caplen as usize > packet.data.len() {
        return false;
    }
    // SAFETY: the program is exclusively library-compiled, not user-supplied
    // bytecode; packet.data contains at least caplen readable bytes. The
    // interpreter bounds packet accesses using caplen, but BPF_LEN must see
    // the ORIGINAL wire length, so pass the unchanged header, not data.len().
    unsafe { (program.api.offline_filter)(&program.raw, packet.header, packet.data.as_ptr()) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    #[test]
    fn npcap_abi() {
        assert_eq!(size_of::<c_int>(), 4);
        assert_eq!(size_of::<c_uint>(), 4);
        assert_eq!(size_of::<libc::c_long>(), 4);
        assert_eq!(size_of::<libc::timeval>(), 8);
        assert_eq!(align_of::<libc::timeval>(), 4);
        assert_eq!(offset_of!(libc::timeval, tv_sec), 0);
        assert_eq!(offset_of!(libc::timeval, tv_usec), 4);
        assert_eq!(size_of::<PacketHeader>(), 16);
        assert_eq!(align_of::<PacketHeader>(), 4);
        assert_eq!(offset_of!(PacketHeader, ts), 0);
        assert_eq!(offset_of!(PacketHeader, caplen), 8);
        assert_eq!(offset_of!(PacketHeader, len), 12);
        assert_eq!(size_of::<BpfInsn>(), 8);
        assert_eq!(align_of::<BpfInsn>(), 4);
        assert_eq!(offset_of!(BpfInsn, code), 0);
        assert_eq!(offset_of!(BpfInsn, jt), 2);
        assert_eq!(offset_of!(BpfInsn, jf), 3);
        assert_eq!(offset_of!(BpfInsn, k), 4);
        let pointer_size = size_of::<*mut BpfInsn>();
        assert_eq!(size_of::<RawBpfProgram>(), 2 * pointer_size);
        assert_eq!(align_of::<RawBpfProgram>(), pointer_size);
        assert_eq!(offset_of!(RawBpfProgram, bf_len), 0);
        assert_eq!(offset_of!(RawBpfProgram, bf_insns), pointer_size);
    }

    #[test]
    fn missing_library_fails_without_fallback() {
        let directory = tempfile::tempdir().unwrap();
        // An absent file in a unique directory fails even if Npcap is
        // installed elsewhere. No dependency on the process-global cache.
        let result = Api::load_from(&directory.path().join("wpcap.dll"));
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("Npcap is unavailable"));
        assert!(Api::load_from(Path::new("wpcap.dll")).is_err());
        assert_eq!(LOAD_FLAGS, 0x00000100 | 0x00000800);
    }

    #[test]
    fn invalid_library_fails_gracefully() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wpcap.dll");
        std::fs::write(&path, b"not a Windows DLL").unwrap();
        assert!(Api::load_from(&path).is_err());
    }

    #[test]
    fn missing_symbols_fail_gracefully() {
        // A known, trusted Windows DLL, but not an implementation of pcap.
        // This exercises cleanup after LoadLibraryExW succeeds.
        let path = system_directory().unwrap().join("kernel32.dll");
        let error = Api::load_from(&path).err().expect("not an Npcap DLL");
        assert!(error.contains("incompatible Npcap library"), "{error}");
    }

    #[test]
    fn filenames_are_never_lossily_converted() {
        assert!(filename(Path::new(r"C:\captures\example.pcap")).is_ok());
        for path in ["capture-\u{00e9}.pcap", "capture-\u{6d4b}\u{8bd5}.pcap"] {
            assert_eq!(
                filename(Path::new(path)).unwrap().to_bytes(),
                path.as_bytes()
            );
        }
        for path in ["a\0b", "-"] {
            assert!(filename(Path::new(path)).is_err());
        }
        let unpaired_surrogate = OsString::from_wide(&[0xd800]);
        assert!(filename(Path::new(&unpaired_surrogate)).is_err());
    }

    #[test]
    fn error_buffer_is_bounded() {
        assert_eq!(error_buffer(&[0; ERRBUF_SIZE]), "");
        assert_eq!(
            error_buffer(&[b'x' as c_char; ERRBUF_SIZE]).len(),
            ERRBUF_SIZE
        );
    }

    fn fixture(data: &[u8], wire_length: u32) -> Vec<u8> {
        let mut bytes = crate::util::pcap::create_header_with_snaplen(1, 65535);
        bytes.extend(crate::util::pcap::create_record_raw(
            1_700_000_000,
            123456,
            wire_length,
            data,
        ));
        bytes
    }

    #[test]
    #[ignore = "requires installed Npcap"]
    fn npcap_unicode_filename() {
        ensure_available().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("capture-\u{00e9}-\u{6d4b}\u{8bd5}.pcap");
        std::fs::write(&path, fixture(&[0; 14], 100)).unwrap();
        let mut capture = Capture::from_file(&path).unwrap();
        assert_eq!(capture.next_packet().unwrap().header.len, 100);
        assert!(matches!(capture.next_packet(), Err(Error::NoMorePackets)));
        drop(capture);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires installed Npcap"]
    fn npcap_offline_read_filter_and_eof() {
        ensure_available().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("offline.pcap");
        let mut ethernet = [0; 14];
        ethernet[12..14].copy_from_slice(&[0x08, 0x00]);
        std::fs::write(&path, fixture(&ethernet, 100)).unwrap();
        let mut capture = Capture::from_file(&path).unwrap();
        assert_eq!(capture.get_datalink(), Linktype::ETHERNET);
        let filter = capture
            .compile("len = 100 and ether proto 0x0800", true)
            .unwrap();
        let wrong_length = capture.compile("len = 14", false).unwrap();
        let unavailable_byte = capture.compile("ether[20] = 0", true).unwrap();
        assert!(capture.compile("this is not valid bpf (", true).is_err());
        assert!(capture.compile("len\0 = 1", true).is_err());
        {
            let packet = capture.next_packet().unwrap();
            assert_eq!(packet.header.ts.tv_sec, 1_700_000_000);
            assert_eq!(packet.header.ts.tv_usec, 123456);
            assert_eq!(packet.header.caplen, 14);
            assert_eq!(packet.header.len, 100);
            assert_eq!(packet.data, ethernet);
            assert!(bpf_matches(&filter, &packet));
            assert!(!bpf_matches(&wrong_length, &packet));
            assert!(!bpf_matches(&unavailable_byte, &packet));
            // Forged Rust metadata must not cause the C filter to overread.
            assert!(!bpf_matches(&filter, &Packet::new(packet.header, &[])));
        }
        assert!(matches!(capture.next_packet(), Err(Error::NoMorePackets)));
        drop(capture);
        // Programs own their instructions independently of their capture.
        let dead = Capture::dead(Linktype::ETHERNET).unwrap();
        let filter = dead.compile("len = 100", true).unwrap();
        drop(dead);
        let header = PacketHeader {
            ts: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            caplen: 0,
            len: 100,
        };
        assert!(bpf_matches(&filter, &Packet::new(&header, &[])));
        // A closed capture must have released the file handle.
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires installed Npcap"]
    fn npcap_fetch_explicit_files_filters_and_preserves_record() {
        use crate::pcap::{PcapFilter, PcapRequest, PcapSource, fetch};
        use crate::util::pcap::{create_header, create_header_with_snaplen, create_record_raw};

        ensure_available().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.pcap");
        let mut ethernet = [0; 14];
        ethernet[12..14].copy_from_slice(&[0x08, 0x00]);
        let mut input = create_header_with_snaplen(1, 65535);
        // Only the third packet passes BOTH the inclusive time window and
        // BPF. All records are truncated (caplen 14, original length 99/100).
        for (micros, wire_length) in [(123455, 100), (123456, 99), (123456, 100), (123457, 100)] {
            input.extend(create_record_raw(
                1_700_000_000,
                micros,
                wire_length,
                &ethernet,
            ));
        }
        std::fs::write(&path, input).unwrap();
        let timestamp = 1_700_000_000_123_456;
        let request = PcapRequest {
            filter: Some(PcapFilter::Expression(
                "len = 100 and ether proto 0x0800".into(),
            )),
            start: Some(timestamp),
            end: Some(timestamp),
            ..Default::default()
        };
        let mut output = Vec::new();
        let stats = fetch(
            &PcapSource::Files(vec![path]),
            &request,
            &mut output,
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(stats.packets, 1);
        assert_eq!(stats.files_scanned, 1);
        assert_eq!(stats.bytes, output.len() as u64);
        assert!(!stats.truncated);
        let mut expected = create_header(1);
        expected.extend(create_record_raw(1_700_000_000, 123456, 100, &ethernet));
        // Exact comparison includes timestamp, original len, caplen, payload,
        // and the writer's global header, not merely the packet count.
        assert_eq!(output, expected);
    }

    #[test]
    #[ignore = "requires installed Npcap"]
    fn npcap_malformed_files() {
        ensure_available().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("malformed.pcap");
        std::fs::write(&path, b"not a pcap file").unwrap();
        assert!(matches!(
            Capture::from_file(&path),
            Err(Error::PcapError(_))
        ));
        let mut truncated = fixture(&[0; 14], 100);
        truncated.pop();
        std::fs::write(&path, truncated).unwrap();
        let mut capture = Capture::from_file(&path).unwrap();
        assert!(matches!(capture.next_packet(), Err(Error::PcapError(_))));
        drop(capture);
        std::fs::remove_file(path).unwrap();
    }
}
