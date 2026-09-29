// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Pure checks comparing what an extracted file claims to be (its wire
// name, the HTTP content type, Suricata's libmagic description) with
// what its leading bytes show. Each check only warns when both sides are
// known.

import type { DetectedType, FileKind } from "./filetype";

// Extensions whose files are expected to be executable code.
const EXECUTABLE_EXTENSIONS = new Set([
  "exe",
  "dll",
  "sys",
  "scr",
  "com",
  "cpl",
  "ocx",
  "drv",
  "efi",
  "so",
  "dylib",
  "elf",
  "bin",
  "class",
]);

// The kinds a file with a (non-executable) extension is expected to be.
const EXTENSION_KINDS: Record<string, FileKind[]> = {
  pdf: ["document"],
  doc: ["document"],
  xls: ["document"],
  ppt: ["document"],
  msi: ["document"],
  docx: ["archive", "document"],
  xlsx: ["archive", "document"],
  pptx: ["archive", "document"],
  odt: ["archive", "document"],
  ods: ["archive", "document"],
  odp: ["archive", "document"],
  rtf: ["text", "document"],
  jpg: ["image"],
  jpeg: ["image"],
  png: ["image"],
  gif: ["image"],
  bmp: ["image"],
  ico: ["image"],
  webp: ["image"],
  svg: ["image", "text"],
  txt: ["text"],
  htm: ["text"],
  html: ["text"],
  css: ["text"],
  js: ["text"],
  json: ["text"],
  xml: ["text"],
  csv: ["text"],
  log: ["text"],
  ini: ["text"],
  md: ["text"],
  zip: ["archive"],
  gz: ["archive"],
  tgz: ["archive"],
  bz2: ["archive"],
  xz: ["archive"],
  "7z": ["archive"],
  rar: ["archive"],
  jar: ["archive", "executable"],
  apk: ["archive"],
  mp4: ["media"],
  m4v: ["media"],
  m4a: ["media"],
  mov: ["media"],
  mkv: ["media"],
  webm: ["media"],
  avi: ["media"],
  flv: ["media"],
  wmv: ["media"],
  wma: ["media"],
  mpg: ["media"],
  mpeg: ["media"],
  m2ts: ["media"],
  // HLS segments, but also TypeScript source.
  ts: ["media", "text"],
  wav: ["media"],
  mp3: ["media"],
  flac: ["media"],
  ogg: ["media"],
  oga: ["media"],
  ogv: ["media"],
  m3u8: ["text"],
};

const KIND_NAMES: Record<FileKind, string> = {
  executable: "an executable",
  archive: "an archive",
  document: "a document",
  image: "an image",
  media: "audio or video",
  text: "text",
  unknown: "unknown content",
};

// The lowercase extension of a wire file name, ignoring any directory,
// query string or fragment. Undefined when there is none.
export function fileExtension(
  filename: string | undefined,
): string | undefined {
  if (!filename) return undefined;
  let name = filename;
  const cut = name.search(/[?#]/);
  if (cut >= 0) name = name.slice(0, cut);
  name = name.slice(
    Math.max(name.lastIndexOf("/"), name.lastIndexOf("\\")) + 1,
  );
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return undefined;
  const ext = name.slice(dot + 1).toLowerCase();
  return /^[a-z0-9]{1,8}$/.test(ext) ? ext : undefined;
}

// The kinds an HTTP content type suggests, or null when it says nothing
// useful (missing, or a generic binary type).
export function contentTypeKinds(contentType: string): FileKind[] | null {
  const t = contentType.split(";")[0].trim().toLowerCase();
  if (!t || t === "application/octet-stream" || t === "binary/octet-stream") {
    return null;
  }
  if (t === "image/svg+xml") return ["image", "text"];
  if (t.startsWith("image/")) return ["image"];
  if (t.startsWith("text/")) return ["text"];
  if (t === "application/pdf") return ["document"];
  // Before the text types: OOXML content types contain "xml".
  if (/msword|ms-excel|ms-powerpoint|officedocument|opendocument/.test(t)) {
    return ["document", "archive"];
  }
  if (
    /json|javascript|ecmascript|xml|x-sh$|x-www-form-urlencoded|mpegurl/.test(t)
  ) {
    return ["text"];
  }
  if (
    t.startsWith("video/") ||
    t.startsWith("audio/") ||
    t === "application/ogg" ||
    t === "application/mp4"
  ) {
    return ["media"];
  }
  if (
    /zip|gzip|x-tar|x-7z|x-rar|x-bzip|x-xz|java-archive|android\.package-archive/.test(
      t,
    )
  ) {
    return ["archive"];
  }
  if (
    /x-msdownload|x-dosexec|x-executable|x-mach-binary|x-elf|portable-executable|x-msdos-program/.test(
      t,
    )
  ) {
    return ["executable"];
  }
  return null;
}

// Specific content types whose files start with a signature detectType
// knows. When one is claimed but no signature is found, the content is
// not what it says: possibly encrypted, disguised or damaged. Types that
// detectType cannot recognise (MP3 without a tag, AAC, BMP, ...) are
// left out so they never warn.
const SIGNED_CONTENT_TYPES = new Set([
  "application/pdf",
  "application/zip",
  "application/gzip",
  "application/x-gzip",
  "application/x-7z-compressed",
  "application/vnd.rar",
  "application/x-rar-compressed",
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/tiff",
  "image/avif",
  "image/heic",
  "video/mp2t",
  "video/mp4",
  "video/quicktime",
  "video/webm",
  "video/x-matroska",
  "video/x-flv",
  "video/x-msvideo",
  "video/x-ms-asf",
  "video/x-ms-wmv",
  "video/ogg",
  "audio/ogg",
  "audio/flac",
  "audio/wav",
  "audio/x-wav",
  "audio/mp4",
  "application/ogg",
  "application/mp4",
]);

// Families recognisable both in a libmagic description and in a
// detected label.
const MAGIC_FAMILIES: { name: string; magic: RegExp; label: RegExp }[] = [
  { name: "PE/MS-DOS executable", magic: /PE32|MS-DOS/i, label: /^(PE|MZ) / },
  { name: "ELF", magic: /\bELF\b/, label: /^ELF / },
  { name: "ZIP", magic: /\bZip\b/i, label: /^ZIP / },
  { name: "PDF", magic: /\bPDF\b/, label: /^PDF / },
  { name: "PNG", magic: /\bPNG\b/, label: /^PNG / },
  { name: "JPEG", magic: /\bJPEG\b/, label: /^JPEG / },
  { name: "GIF", magic: /\bGIF\b/, label: /^GIF / },
  {
    name: "MPEG transport stream",
    magic: /MPEG transport stream/i,
    label: /^MPEG transport /,
  },
  {
    name: "MP4/QuickTime",
    magic: /ISO Media|Apple QuickTime/i,
    label: /^(MP4|QuickTime) /,
  },
  { name: "Matroska/WebM", magic: /Matroska|WebM/i, label: /^Matroska/ },
  { name: "Ogg", magic: /\bOgg\b/, label: /^Ogg / },
  { name: "FLAC", magic: /\bFLAC\b/, label: /^FLAC / },
  { name: "WAV", magic: /WAVE audio/i, label: /^WAV / },
  { name: "AVI", magic: /\bAVI\b/, label: /^AVI / },
];

export interface WarningInput {
  // The file name seen on the wire.
  filename?: string;
  // The HTTP content type of the transaction carrying the file.
  contentType?: string;
  // Suricata's libmagic description.
  magic?: string;
  // From the event's fileinfo.
  gaps?: boolean;
  state?: string;
  // What the loaded bytes show.
  detected: DetectedType | null;
  isText: boolean;
}

// Human-readable warnings, as plain text.
export function fileWarnings(input: WarningInput): string[] {
  const warnings: string[] = [];
  const detected = input.detected;
  // The kind the content shows: a signature if one matched, else text
  // when it looks like text. Plain binary with no signature is unknown.
  const kind: FileKind | null =
    detected && detected.kind !== "unknown"
      ? detected.kind
      : input.isText
        ? "text"
        : null;
  const what = detected ? detected.label : "text";

  const ext = fileExtension(input.filename);
  if (kind && ext) {
    if (kind === "executable" && !EXECUTABLE_EXTENSIONS.has(ext)) {
      const expected = EXTENSION_KINDS[ext];
      if (!expected || !expected.includes("executable")) {
        warnings.push(`File name ends in .${ext} but the content is ${what}.`);
      }
    } else {
      const expected = EXECUTABLE_EXTENSIONS.has(ext)
        ? (["executable"] as FileKind[])
        : EXTENSION_KINDS[ext];
      if (expected && !expected.includes(kind)) {
        warnings.push(
          `File name ends in .${ext}, suggesting ${KIND_NAMES[expected[0]]}, but the content is ${what}.`,
        );
      }
    }
  }

  if (kind && input.contentType) {
    const expected = contentTypeKinds(input.contentType);
    if (expected && !expected.includes(kind)) {
      warnings.push(
        `HTTP content type ${input.contentType} suggests ${KIND_NAMES[expected[0]]}, but the content is ${what}.`,
      );
    }
  } else if (!kind && input.contentType) {
    const type = input.contentType.split(";")[0].trim().toLowerCase();
    const expected = contentTypeKinds(type);
    if (expected && SIGNED_CONTENT_TYPES.has(type)) {
      warnings.push(
        `HTTP content type ${input.contentType} suggests ${KIND_NAMES[expected[0]]}, but the content has no matching signature; it may be encrypted, disguised or damaged.`,
      );
    }
  }

  if (input.magic && detected) {
    const fromMagic = MAGIC_FAMILIES.find((f) => f.magic.test(input.magic!));
    const fromBytes = MAGIC_FAMILIES.find((f) => f.label.test(detected.label));
    if (fromMagic && fromBytes && fromMagic !== fromBytes) {
      warnings.push(
        `Suricata identified the file as ${fromMagic.name} but the content is ${detected.label}.`,
      );
    }
  }

  if (
    input.gaps === true ||
    (typeof input.state === "string" && input.state !== "CLOSED")
  ) {
    warnings.push("File is incomplete; hashes will not match known samples.");
  }

  return warnings;
}
