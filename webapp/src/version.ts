// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Minimal SemVer parsing/comparison, just enough for the update check. Mirrors
// the Rust `semver` crate's precedence rules for the cases EveBox produces:
// MAJOR.MINOR.PATCH with an optional `-prerelease` tag (e.g. "0.26.0-dev").

export interface ParsedVersion {
  major: number;
  minor: number;
  patch: number;
  // Pre-release tag without the leading '-', or "" if this is a release.
  pre: string;
}

export function parseVersion(input: string): ParsedVersion | null {
  const m = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?/.exec(input.trim());
  if (!m) {
    return null;
  }
  return {
    major: parseInt(m[1], 10),
    minor: parseInt(m[2], 10),
    patch: parseInt(m[3], 10),
    pre: m[4] ?? "",
  };
}

// Compare two version strings. Returns a negative number if a < b, 0 if equal,
// a positive number if a > b, or null if either could not be parsed.
//
// Per SemVer, a pre-release version is lower than its associated release, so
// "0.26.0-dev" < "0.26.0", while "0.26.0-dev" > "0.25.0".
export function compareVersions(a: string, b: string): number | null {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  if (!pa || !pb) {
    return null;
  }
  if (pa.major !== pb.major) return pa.major - pb.major;
  if (pa.minor !== pb.minor) return pa.minor - pb.minor;
  if (pa.patch !== pb.patch) return pa.patch - pb.patch;
  if (pa.pre === pb.pre) return 0;
  // Same core version: the one without a pre-release tag is greater.
  if (pa.pre === "") return 1;
  if (pb.pre === "") return -1;
  return pa.pre < pb.pre ? -1 : 1;
}

// True if the version string carries a pre-release tag (e.g. a "-dev" build).
export function isPrerelease(input: string): boolean {
  return parseVersion(input)?.pre !== "";
}
