// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Pure string extraction and classification helpers.

export interface ExtractedString {
  // Absolute offset of the first byte of the string.
  offset: number;
  // "A" for ASCII, "W" for UTF-16LE.
  encoding: "A" | "W";
  // The string value, truncated to MAX_STRING_LEN characters with a
  // trailing TRUNCATION_MARKER if longer.
  value: string;
}

export interface ExtractStringsResult {
  strings: ExtractedString[];
  // True if extraction stopped after MAX_STRINGS results.
  truncated: boolean;
}

export const MAX_STRING_LEN = 1024;
export const MAX_STRINGS = 10000;
export const TRUNCATION_MARKER = "\u2026";

export const STRING_TAGS = [
  "url",
  "ipv4",
  "domain",
  "email",
  "path",
  "registry",
  "base64",
  "api",
] as const;

export type StringTag = (typeof STRING_TAGS)[number];

function isStringByte(b: number): boolean {
  return (b >= 0x20 && b <= 0x7e) || b === 0x09;
}

// Accumulates characters of a run, keeping at most MAX_STRING_LEN.
class Run {
  start = -1;
  len = 0;
  chars: number[] = [];

  push(offset: number, b: number) {
    if (this.len === 0) {
      this.start = offset;
    }
    if (this.len < MAX_STRING_LEN) {
      this.chars.push(b);
    }
    this.len++;
  }

  // Emits the run if long enough and resets. Returns false once `out` is
  // full.
  flush(
    out: ExtractedString[],
    encoding: "A" | "W",
    baseOffset: number,
    minLen: number,
  ): boolean {
    if (this.len >= minLen) {
      if (out.length >= MAX_STRINGS) {
        this.reset();
        return false;
      }
      let value = String.fromCharCode.apply(null, this.chars);
      if (this.len > MAX_STRING_LEN) {
        value += TRUNCATION_MARKER;
      }
      out.push({ offset: baseOffset + this.start, encoding, value });
    }
    this.reset();
    return true;
  }

  reset() {
    this.start = -1;
    this.len = 0;
    this.chars = [];
  }
}

function extractAscii(
  bytes: Uint8Array,
  baseOffset: number,
  minLen: number,
  out: ExtractedString[],
): boolean {
  const run = new Run();
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i];
    if (isStringByte(b)) {
      run.push(i, b);
    } else if (!run.flush(out, "A", baseOffset, minLen)) {
      return false;
    }
  }
  return run.flush(out, "A", baseOffset, minLen);
}

function extractUtf16le(
  bytes: Uint8Array,
  baseOffset: number,
  minLen: number,
  align: number,
  out: ExtractedString[],
): boolean {
  const run = new Run();
  for (let i = align; i + 1 < bytes.length; i += 2) {
    const b = bytes[i];
    if (isStringByte(b) && bytes[i + 1] === 0) {
      run.push(i, b);
    } else if (!run.flush(out, "W", baseOffset, minLen)) {
      return false;
    }
  }
  return run.flush(out, "W", baseOffset, minLen);
}

// Extract printable ASCII and UTF-16LE strings of at least `minLen`
// characters. Results are sorted by offset and limited to MAX_STRINGS.
export function extractStrings(
  bytes: Uint8Array,
  baseOffset: number,
  minLen: number,
): ExtractStringsResult {
  minLen = Math.max(1, Math.floor(minLen));

  // Each pass keeps its first MAX_STRINGS results by offset, so the first
  // MAX_STRINGS of the combined, sorted list are exact.
  const ascii: ExtractedString[] = [];
  const even: ExtractedString[] = [];
  const odd: ExtractedString[] = [];
  let complete = extractAscii(bytes, baseOffset, minLen, ascii);
  // UTF-16LE strings may start on an even or odd byte offset.
  complete = extractUtf16le(bytes, baseOffset, minLen, 0, even) && complete;
  complete = extractUtf16le(bytes, baseOffset, minLen, 1, odd) && complete;

  const all = ascii.concat(even, odd);
  all.sort(
    (a, b) =>
      a.offset - b.offset ||
      (a.encoding < b.encoding ? -1 : a.encoding > b.encoding ? 1 : 0),
  );
  const truncated = !complete || all.length > MAX_STRINGS;
  if (all.length > MAX_STRINGS) {
    all.length = MAX_STRINGS;
  }
  return { strings: all, truncated };
}

// Classification is only applied to strings up to this length (allowing
// for the truncation marker).
const MAX_CLASSIFY_LEN = MAX_STRING_LEN + TRUNCATION_MARKER.length;

const URL_RE = /\bhttps?:\/\/[^\s"'<>]/i;
const IPV4_RE = /\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b/g;
const DOMAIN_RE = /\b[A-Za-z0-9][A-Za-z0-9.-]*\.([a-z]{2,12})\b/g;
const EMAIL_RE = /[A-Za-z0-9._%+-]@[A-Za-z0-9-]+\.[A-Za-z]/;
const WIN_PATH_RE = /\b[A-Za-z]:\\/;
const UNC_PATH_RE = /\\\\[A-Za-z0-9._$-]+\\/;
const UNIX_PATH_RE = /(?:^|[\s"'=(])\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]/;
const REGISTRY_RE = /\b(?:HKLM|HKCU|HKCR|HKU|HKCC)\\|\bHKEY_[A-Z]/i;
const BASE64_RE = /[A-Za-z0-9+/]{40}/;
const API_RE =
  /\b(?:VirtualAlloc|VirtualProtect|WriteProcessMemory|CreateRemoteThread|WinExec|ShellExecute|URLDownloadToFile|LoadLibrary|GetProcAddress|InternetOpen|CreateProcess)/;

// File extensions commonly found in binaries that would otherwise look
// like domain names.
const NOT_TLDS = new Set([
  "bat",
  "bin",
  "cmd",
  "cpl",
  "dat",
  "dll",
  "drv",
  "exe",
  "h",
  "htm",
  "html",
  "ini",
  "js",
  "json",
  "log",
  "ocx",
  "pdb",
  "ps1",
  "scr",
  "sys",
  "tmp",
  "txt",
  "xml",
]);

function hasIpv4(value: string): boolean {
  for (const m of value.matchAll(IPV4_RE)) {
    if (
      Number(m[1]) <= 255 &&
      Number(m[2]) <= 255 &&
      Number(m[3]) <= 255 &&
      Number(m[4]) <= 255
    ) {
      return true;
    }
  }
  return false;
}

function hasDomain(value: string): boolean {
  for (const m of value.matchAll(DOMAIN_RE)) {
    if (!NOT_TLDS.has(m[1])) {
      return true;
    }
  }
  return false;
}

// Return the tags (from STRING_TAGS, in order) that apply to a string.
export function classify(value: string): StringTag[] {
  const tags: StringTag[] = [];
  if (value.length > MAX_CLASSIFY_LEN) {
    return tags;
  }
  if (URL_RE.test(value)) {
    tags.push("url");
  }
  if (hasIpv4(value)) {
    tags.push("ipv4");
  }
  if (hasDomain(value)) {
    tags.push("domain");
  }
  if (value.includes("@") && EMAIL_RE.test(value)) {
    tags.push("email");
  }
  if (
    WIN_PATH_RE.test(value) ||
    UNC_PATH_RE.test(value) ||
    UNIX_PATH_RE.test(value)
  ) {
    tags.push("path");
  }
  if (REGISTRY_RE.test(value)) {
    tags.push("registry");
  }
  if (BASE64_RE.test(value)) {
    tags.push("base64");
  }
  if (API_RE.test(value)) {
    tags.push("api");
  }
  return tags;
}
