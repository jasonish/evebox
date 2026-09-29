// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Pure text decoding helpers.

export type TextEncodingChoice =
  | "auto"
  | "utf-8"
  | "utf-16le"
  | "utf-16be"
  | "windows-1252";

export interface DecodedText {
  text: string;
  encoding: string;
}

function decodeWith(bytes: Uint8Array, encoding: string): string {
  // ignoreBOM defaults to false, so a matching BOM is stripped.
  return new TextDecoder(encoding, { fatal: false }).decode(bytes);
}

function bomEncoding(bytes: Uint8Array): string | null {
  if (
    bytes.length >= 3 &&
    bytes[0] === 0xef &&
    bytes[1] === 0xbb &&
    bytes[2] === 0xbf
  ) {
    return "utf-8";
  }
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return "utf-16le";
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    return "utf-16be";
  }
  return null;
}

// Decode bytes to text. With "auto" a BOM decides the encoding, otherwise
// UTF-8 is tried and windows-1252 is used if more than 1% of the decoded
// characters are replacement characters.
export function decodeText(
  bytes: Uint8Array,
  encoding: TextEncodingChoice = "auto",
): DecodedText {
  if (encoding !== "auto") {
    return { text: decodeWith(bytes, encoding), encoding };
  }

  const bom = bomEncoding(bytes);
  if (bom) {
    return { text: decodeWith(bytes, bom), encoding: bom };
  }

  const text = decodeWith(bytes, "utf-8");
  let replacements = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 0xfffd) {
      replacements++;
    }
  }
  if (replacements * 100 > text.length) {
    return {
      text: decodeWith(bytes, "windows-1252"),
      encoding: "windows-1252",
    };
  }
  return { text, encoding: "utf-8" };
}

// C0 controls (except tab, LF, CR), DEL, bidi controls and zero-width
// characters.
const HIDDEN_RE =
  /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f\u200b-\u200d\u202a-\u202e\u2060\u2066-\u2069\ufeff]/g;

// Make hidden characters visible. C0 controls and DEL become Unicode
// Control Pictures; bidi and zero-width characters become a marker such as
// "⟦U+202E⟧". A leading U+FEFF (BOM) is left as is.
export function visibleControls(text: string): string {
  return text.replace(HIDDEN_RE, (ch: string, offset: number) => {
    const code = ch.charCodeAt(0);
    if (code < 0x20) {
      return String.fromCharCode(0x2400 + code);
    }
    if (code === 0x7f) {
      return "\u2421";
    }
    if (code === 0xfeff && offset === 0) {
      return ch;
    }
    return `\u27e6U+${code.toString(16).toUpperCase().padStart(4, "0")}\u27e7`;
  });
}
