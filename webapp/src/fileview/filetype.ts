// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Pure file type detection helpers based on magic bytes.

export type FileKind =
  | "executable"
  | "archive"
  | "document"
  | "image"
  | "media"
  | "text"
  | "unknown";

export interface DetectedType {
  label: string;
  kind: FileKind;
}

interface Signature {
  magic: number[];
  label: string;
  kind: FileKind;
}

// Fixed signatures checked at offset 0. MZ is handled separately.
const SIGNATURES: Signature[] = [
  {
    magic: [0x7f, 0x45, 0x4c, 0x46],
    label: "ELF executable",
    kind: "executable",
  },
  {
    magic: [0xfe, 0xed, 0xfa, 0xce],
    label: "Mach-O executable",
    kind: "executable",
  },
  {
    magic: [0xfe, 0xed, 0xfa, 0xcf],
    label: "Mach-O executable",
    kind: "executable",
  },
  {
    magic: [0xce, 0xfa, 0xed, 0xfe],
    label: "Mach-O executable",
    kind: "executable",
  },
  {
    magic: [0xcf, 0xfa, 0xed, 0xfe],
    label: "Mach-O executable",
    kind: "executable",
  },
  {
    magic: [0xca, 0xfe, 0xba, 0xbe],
    label: "Mach-O fat binary / Java class",
    kind: "executable",
  },
  {
    magic: [0x50, 0x4b, 0x03, 0x04],
    label: "ZIP archive (also JAR, OOXML, APK)",
    kind: "archive",
  },
  {
    magic: [0x50, 0x4b, 0x05, 0x06],
    label: "ZIP archive (empty)",
    kind: "archive",
  },
  {
    magic: [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1],
    label: "OLE2 compound document (legacy Office, MSI)",
    kind: "document",
  },
  {
    magic: [0x25, 0x50, 0x44, 0x46, 0x2d],
    label: "PDF document",
    kind: "document",
  },
  { magic: [0x1f, 0x8b], label: "gzip compressed data", kind: "archive" },
  {
    magic: [0x42, 0x5a, 0x68],
    label: "bzip2 compressed data",
    kind: "archive",
  },
  {
    magic: [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00],
    label: "xz compressed data",
    kind: "archive",
  },
  {
    magic: [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c],
    label: "7-Zip archive",
    kind: "archive",
  },
  {
    magic: [0x52, 0x61, 0x72, 0x21, 0x1a, 0x07],
    label: "RAR archive",
    kind: "archive",
  },
  {
    magic: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    label: "PNG image",
    kind: "image",
  },
  { magic: [0xff, 0xd8, 0xff], label: "JPEG image", kind: "image" },
  { magic: [0x47, 0x49, 0x46, 0x38], label: "GIF image", kind: "image" },
  { magic: [0x49, 0x49, 0x2a, 0x00], label: "TIFF image", kind: "image" },
  { magic: [0x4d, 0x4d, 0x00, 0x2a], label: "TIFF image", kind: "image" },
  {
    magic: [0x1a, 0x45, 0xdf, 0xa3],
    label: "Matroska/WebM media",
    kind: "media",
  },
  { magic: [0x46, 0x4c, 0x56, 0x01], label: "FLV video", kind: "media" },
  { magic: [0x4f, 0x67, 0x67, 0x53], label: "Ogg media", kind: "media" },
  { magic: [0x66, 0x4c, 0x61, 0x43], label: "FLAC audio", kind: "media" },
  { magic: [0x49, 0x44, 0x33], label: "MP3 audio (ID3 tag)", kind: "media" },
  {
    magic: [0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11],
    label: "ASF media (WMV, WMA)",
    kind: "media",
  },
  {
    magic: [0x00, 0x00, 0x01, 0xba],
    label: "MPEG program stream",
    kind: "media",
  },
  { magic: [0xef, 0xbb, 0xbf], label: "UTF-8 text (BOM)", kind: "text" },
  { magic: [0xff, 0xfe], label: "UTF-16LE text (BOM)", kind: "text" },
  { magic: [0xfe, 0xff], label: "UTF-16BE text (BOM)", kind: "text" },
  { magic: [0x23, 0x21], label: "Script (#!)", kind: "text" },
];

const MARKUP: { prefix: string; label: string }[] = [
  { prefix: "<?xml", label: "XML markup" },
  { prefix: "<!doctype html", label: "HTML markup" },
  { prefix: "<html", label: "HTML markup" },
  { prefix: "<svg", label: "SVG markup" },
];

function startsWith(bytes: Uint8Array, magic: number[], at = 0): boolean {
  if (bytes.length - at < magic.length) {
    return false;
  }
  for (let i = 0; i < magic.length; i++) {
    if (bytes[at + i] !== magic[i]) {
      return false;
    }
  }
  return true;
}

// Case-insensitive ASCII prefix match; `prefix` must be lowercase.
function startsWithAsciiNoCase(
  bytes: Uint8Array,
  prefix: string,
  at: number,
): boolean {
  if (bytes.length - at < prefix.length) {
    return false;
  }
  for (let i = 0; i < prefix.length; i++) {
    let b = bytes[at + i];
    if (b >= 0x41 && b <= 0x5a) {
      b += 0x20;
    }
    if (b !== prefix.charCodeAt(i)) {
      return false;
    }
  }
  return true;
}

function ascii(bytes: Uint8Array, at: number, length: number): string {
  if (bytes.length < at + length) {
    return "";
  }
  return String.fromCharCode(...bytes.subarray(at, at + length));
}

// Transport stream packets start with a 0x47 sync byte: every 188 bytes,
// or every 192 in M2TS (Blu-ray, AVCHD), which prefixes each packet with
// a 4-byte timestamp. A single 0x47 is too common to mean anything, so
// require at least three packets, checking up to eight.
function isTransportStream(bytes: Uint8Array): boolean {
  for (const [first, stride] of [
    [0, 188],
    [4, 192],
  ]) {
    const packets = Math.min(8, Math.floor((bytes.length - first) / stride));
    if (packets < 3) {
      continue;
    }
    let ok = true;
    for (let p = 0; ok && p < packets; p++) {
      ok = bytes[first + p * stride] === 0x47;
    }
    if (ok) {
      return true;
    }
  }
  return false;
}

// RIFF containers: the form type at offset 8 says what is inside.
function detectRiff(bytes: Uint8Array): DetectedType | null {
  if (ascii(bytes, 0, 4) !== "RIFF") {
    return null;
  }
  switch (ascii(bytes, 8, 4)) {
    case "WAVE":
      return { label: "WAV audio", kind: "media" };
    case "AVI ":
      return { label: "AVI video", kind: "media" };
    case "WEBP":
      return { label: "WebP image", kind: "image" };
  }
  return null;
}

// ISO base media files (MP4, QuickTime, HEIF) start with an "ftyp" box;
// its major brand at offset 8 separates images from audio and video.
function detectIsoMedia(bytes: Uint8Array): DetectedType | null {
  if (ascii(bytes, 4, 4) !== "ftyp") {
    return null;
  }
  const brand = ascii(bytes, 8, 4);
  if (/^(avif|avis)$/.test(brand)) {
    return { label: "AVIF image", kind: "image" };
  }
  if (/^(heic|heix|heim|heis|mif1|msf1)$/.test(brand)) {
    return { label: "HEIF image", kind: "image" };
  }
  if (brand === "qt  ") {
    return { label: "QuickTime movie", kind: "media" };
  }
  return { label: "MP4 media (ISO base media)", kind: "media" };
}

function isAsciiWhitespace(b: number): boolean {
  return (
    b === 0x20 ||
    b === 0x09 ||
    b === 0x0a ||
    b === 0x0d ||
    b === 0x0c ||
    b === 0x0b
  );
}

// Detect a file type from the leading bytes. Returns null if unknown.
export function detectType(bytes: Uint8Array): DetectedType | null {
  if (startsWith(bytes, [0x4d, 0x5a])) {
    if (bytes.length >= 0x40) {
      const lfanew =
        (bytes[0x3c] |
          (bytes[0x3d] << 8) |
          (bytes[0x3e] << 16) |
          (bytes[0x3f] << 24)) >>>
        0;
      if (startsWith(bytes, [0x50, 0x45, 0x00, 0x00], lfanew)) {
        return { label: "PE executable", kind: "executable" };
      }
    }
    return { label: "MZ executable", kind: "executable" };
  }

  for (const sig of SIGNATURES) {
    if (startsWith(bytes, sig.magic)) {
      return { label: sig.label, kind: sig.kind };
    }
  }

  const container = detectRiff(bytes) ?? detectIsoMedia(bytes);
  if (container) {
    return container;
  }
  if (isTransportStream(bytes)) {
    return { label: "MPEG transport stream", kind: "media" };
  }

  let i = 0;
  while (i < bytes.length && isAsciiWhitespace(bytes[i])) {
    i++;
  }
  for (const m of MARKUP) {
    if (startsWithAsciiNoCase(bytes, m.prefix, i)) {
      return { label: m.label, kind: "text" };
    }
  }

  return null;
}

const TEXT_SAMPLE_SIZE = 8192;

// Heuristic check for whether the data looks like text. Only the first
// 8 KiB is examined.
export function isProbablyText(bytes: Uint8Array): boolean {
  const n = Math.min(bytes.length, TEXT_SAMPLE_SIZE);
  const utf16Bom =
    n >= 2 &&
    ((bytes[0] === 0xff && bytes[1] === 0xfe) ||
      (bytes[0] === 0xfe && bytes[1] === 0xff));
  if (utf16Bom) {
    return true;
  }

  let bad = 0;
  let i = 0;
  while (i < n) {
    const b = bytes[i];
    if (b === 0) {
      return false;
    }
    if (b === 0x09 || b === 0x0a || b === 0x0d || (b >= 0x20 && b <= 0x7e)) {
      i++;
      continue;
    }
    // Try to match a valid UTF-8 multibyte sequence.
    let need = 0;
    if (b >= 0xc2 && b <= 0xdf) {
      need = 1;
    } else if (b >= 0xe0 && b <= 0xef) {
      need = 2;
    } else if (b >= 0xf0 && b <= 0xf4) {
      need = 3;
    }
    let ok = need > 0;
    for (let j = 1; ok && j <= need; j++) {
      const c = bytes[i + j];
      if (i + j >= n || c < 0x80 || c > 0xbf) {
        ok = false;
      }
    }
    if (ok) {
      i += need + 1;
    } else {
      bad++;
      i++;
    }
  }
  return bad * 10 <= n;
}

// A label for content isProbablyText accepted but no signature matched:
// ASCII when the sample is 7-bit clean, otherwise UTF-8.
export function textLabel(bytes: Uint8Array): string {
  const n = Math.min(bytes.length, TEXT_SAMPLE_SIZE);
  for (let i = 0; i < n; i++) {
    if (bytes[i] >= 0x80) {
      return "UTF-8 text";
    }
  }
  return "ASCII text";
}

// Shannon entropy in bits per byte (0-8).
export function entropy(bytes: Uint8Array): number {
  const n = bytes.length;
  if (n === 0) {
    return 0;
  }
  const counts = new Uint32Array(256);
  for (let i = 0; i < n; i++) {
    counts[bytes[i]]++;
  }
  let h = 0;
  for (let i = 0; i < 256; i++) {
    const c = counts[i];
    if (c > 0) {
      const p = c / n;
      h -= p * Math.log2(p);
    }
  }
  return h;
}
