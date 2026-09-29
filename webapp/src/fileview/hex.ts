// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Pure hex dump helpers.

const HEX: string[] = (() => {
  const out: string[] = new Array(256);
  for (let i = 0; i < 256; i++) {
    out[i] = (i < 16 ? "0" : "") + i.toString(16);
  }
  return out;
})();

function printable(b: number): string {
  return b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : ".";
}

// Decode a base64 string into bytes.
export function base64ToBytes(b64: string): Uint8Array {
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
}

// Produce `hexdump -C` style lines:
//
//   00000000  4d 5a 90 00 03 00 00 00  04 00 00 00 ff ff 00 00  |MZ..............|
//
// The last line is padded so the ASCII column lines up.
export function hexLines(bytes: Uint8Array, baseOffset: number): string[] {
  const lines: string[] = [];
  for (let start = 0; start < bytes.length; start += 16) {
    const end = Math.min(start + 16, bytes.length);
    let hex = "";
    let ascii = "";
    for (let i = 0; i < 16; i++) {
      if (i === 8) {
        hex += " ";
      }
      const idx = start + i;
      if (idx < end) {
        const b = bytes[idx];
        hex += HEX[b] + " ";
        ascii += printable(b);
      } else {
        hex += "   ";
      }
    }
    const offset = (baseOffset + start).toString(16).padStart(8, "0");
    lines.push(`${offset}  ${hex} |${ascii}|`);
  }
  return lines;
}

// Split bytes into rows of 16 as `[hex, printable]` pairs, where hex is
// space separated and printable maps 0x20-0x7E to itself and everything
// else to ".".
export function prettyHex(bytes: Uint8Array): [string, string][] {
  const output: [string, string][] = [];
  for (let start = 0; start < bytes.length; start += 16) {
    const end = Math.min(start + 16, bytes.length);
    const hex: string[] = [];
    let ascii = "";
    for (let i = start; i < end; i++) {
      hex.push(HEX[bytes[i]]);
      ascii += printable(bytes[i]);
    }
    output.push([hex.join(" "), ascii]);
  }
  return output;
}
