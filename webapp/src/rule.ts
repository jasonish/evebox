// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Rule parsing helpers for display purposes. This is not a complete
// Suricata rule parser, just enough to break a rule into its header
// and options for highlighting and to resolve reference links.

// Reference types we know how to turn into a URL, keyed by lower case
// type name. The "reference" keyword is free form, so only well known
// types with a working destination are included here. When an alert
// includes Suricata's resolved "references" those take precedence.
export const RULE_REFERENCE_URLS: Record<string, string> = {
  cve: "https://cve.mitre.org/cgi-bin/cvename.cgi?name=",
  et: "https://doc.emergingthreats.net/",
  etpro: "https://doc.emergingthreatspro.com/",
  exploitdb: "https://www.exploit-db.com/exploits/",
  msft: "https://technet.microsoft.com/security/bulletin/",
  nessus: "https://www.tenable.com/plugins/nessus/",
  url: "https://",
};

export interface RuleOption {
  // Whitespace preceding the option, preserved for display.
  leading: string;
  keyword: string;
  // The option value with the leading ":" removed. Undefined for
  // keywords that take no value, such as "nocase".
  value?: string;
  // The ";" terminating the option, or empty if absent.
  terminator: string;
}

export interface ParsedRule {
  // The 7 rule header fields: action, protocol, source address,
  // source port, direction, destination address, destination port.
  header: string[];
  options: RuleOption[];
}

export interface RuleReference {
  type: string;
  value: string;
  // Resolved URL, undefined if the reference could not be resolved.
  url?: string;
}

// Parse a rule into its header and options. Returns undefined if the
// rule does not have the expected shape, in which case the caller
// should fall back to displaying the rule as plain text.
export function parseRule(rule: string): ParsedRule | undefined {
  const m = rule.match(
    /^\s*(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s*\(([\s\S]*)\)\s*$/,
  );
  if (!m) {
    return undefined;
  }
  return {
    header: m.slice(1, 8),
    options: splitOptions(m[8]),
  };
}

// Split the body of a rule into options on unescaped ";" characters
// that are not inside a quoted string.
function splitOptions(body: string): RuleOption[] {
  const options: RuleOption[] = [];
  let current = "";
  let quoted = false;
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === "\\" && i + 1 < body.length) {
      current += c + body[i + 1];
      i++;
      continue;
    }
    if (c === '"') {
      quoted = !quoted;
    }
    current += c;
    if (c === ";" && !quoted) {
      options.push(parseOption(current));
      current = "";
    }
  }
  if (current.trim()) {
    options.push(parseOption(current));
  }
  return options;
}

function parseOption(segment: string): RuleOption {
  const leading = segment.match(/^\s*/)![0];
  let rest = segment.slice(leading.length);
  let terminator = "";
  if (rest.endsWith(";")) {
    terminator = ";";
    rest = rest.slice(0, -1);
  }
  const colon = rest.indexOf(":");
  if (colon < 0) {
    return { leading, keyword: rest, terminator };
  }
  return {
    leading,
    keyword: rest.slice(0, colon),
    value: rest.slice(colon + 1),
    terminator,
  };
}

function isUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

// Parse the value of a "reference" option, e.g. "cve,2021-44228".
function parseReference(value: string): RuleReference | undefined {
  const comma = value.indexOf(",");
  if (comma < 0) {
    return undefined;
  }
  const type = value.slice(0, comma).trim();
  const ref = value
    .slice(comma + 1)
    .trim()
    .replace(/^"(.*)"$/, "$1");
  if (!type || !ref) {
    return undefined;
  }
  return { type, value: ref };
}

// Resolve a reference to a URL, or undefined if it cannot be resolved.
//
// The "logged" argument is the list of references as logged by
// Suricata in "alert.references", which are already joined with the
// URL prefix from the sensor's reference.config. If one of these ends
// with the reference value it is used as is, otherwise the reference
// is resolved using the known reference types.
function referenceUrl(
  type: string,
  value: string,
  logged: string[],
): string | undefined {
  if (isUrl(value)) {
    return value;
  }
  // The value must sit on a non-alphanumeric boundary so "890" does
  // not match ".../bid/11890", and the shortest match is preferred
  // for a value that is a suffix of another in the same rule.
  const match = logged
    .filter(
      (r) =>
        isUrl(r) &&
        r.endsWith(value) &&
        !/[A-Za-z0-9]/.test(r[r.length - value.length - 1] || ""),
    )
    .sort((a, b) => a.length - b.length)[0];
  if (match) {
    return match;
  }
  const prefix = RULE_REFERENCE_URLS[type.toLowerCase()];
  if (!prefix) {
    return undefined;
  }
  return prefix + value;
}

// The "alert.references" value from an event, which may not be an
// array of strings if the event comes from an unexpected source.
function loggedReferences(logged: unknown): string[] {
  return Array.isArray(logged)
    ? logged.filter((r): r is string => typeof r === "string")
    : [];
}

// Return all references for an alert from the rule text and the
// "alert.references" logged by Suricata, if present.
export function ruleReferences(
  rule: ParsedRule | undefined,
  logged: unknown = [],
): RuleReference[] {
  const resolved = loggedReferences(logged);
  const parsed: RuleReference[] = [];
  for (const option of rule?.options || []) {
    if (option.keyword.toLowerCase() === "reference" && option.value) {
      const reference = parseReference(option.value);
      if (reference) {
        parsed.push(reference);
      }
    }
  }

  const references: RuleReference[] = [];
  const push = (reference: RuleReference) => {
    const duplicate = references.some((r) =>
      r.url
        ? r.url === reference.url
        : r.type === reference.type && r.value === reference.value,
    );
    if (!duplicate) {
      references.push(reference);
    }
  };

  if (resolved.length > 0 && resolved.length === parsed.length) {
    // Suricata logs one entry per reference option in rule order, so
    // pair them by position. An entry that is not a URL, such as a
    // type unknown to the sensor, falls back to the known types.
    parsed.forEach((reference, i) => {
      push({
        ...reference,
        url: isUrl(resolved[i])
          ? resolved[i]
          : referenceUrl(reference.type, reference.value, []),
      });
    });
    return references;
  }

  for (const reference of parsed) {
    push({
      ...reference,
      url: referenceUrl(reference.type, reference.value, resolved),
    });
  }
  for (const value of resolved) {
    if (!references.some((r) => r.url === value)) {
      push({ type: "", value, url: isUrl(value) ? value : undefined });
    }
  }
  return references;
}
