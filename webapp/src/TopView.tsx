// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Top: a live, at-a-glance dashboard fed by the firehose. Unlike the
// Live view, which scrolls raw events, Top aggregates the stream in
// the browser (event types, alerts by severity and category, alert
// hosts, top DNS names and TLS/QUIC SNIs), either since the page was opened or over a
// rolling window, and calls out things that are newly seen: names, SNIs
// and signatures not seen in the datastore over the last 24 hours.
//
// Everything is bucketed by arrival time rather than event timestamp,
// so the counts reflect what this page has received.

import { Top } from "./Top";
import * as API from "./api";
import {
  createEffect,
  createSignal,
  For,
  JSX,
  on,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { A, useSearchParams } from "@solidjs/router";
import { Container } from "solid-bootstrap";
import { SensorSelect } from "./common/SensorSelect";
import { StatCard } from "./components/StatCard";
import { EVENT_TYPES, eventNameFromType } from "./Events";
import { SearchLink } from "./common/SearchLink";
import { compressIpv6 } from "./formatters";
import { BiInfoCircle } from "./icons";
import { Tooltip } from "bootstrap";

// Range of the counts. "all" counts everything since the page was
// opened (or reset); the others are rolling windows.
const WINDOWS = [
  { label: "all", name: "-", secs: 0 },
  { label: "1m", name: "1m", secs: 60 },
  { label: "5m", name: "5m", secs: 300 },
  { label: "15m", name: "15m", secs: 900 },
  { label: "30m", name: "30m", secs: 1800 },
  { label: "1h", name: "1h", secs: 3600 },
];

const RANGE_TOOLTIP =
  'How far back the counts go. "-" counts everything since the page ' +
  "was opened or last reset. The others count only the last 1 minute " +
  "to 1 hour, as a rolling window. Counts are based on when events " +
  "arrive here. All ranges are counted at once, so switching is instant.";
const DEFAULT_WINDOW = "all";

// The range is split into this many buckets; also the sparkline
// resolution.
const BUCKETS = 60;

// Unlimited mode starts with buckets this wide and doubles their width,
// merging neighbours, each time the buckets fill up, so the sparklines
// always span the whole time since opened.
const UNLIMITED_BUCKET_MS = 5000;

// Only these dimensions are kept per bucket in unlimited mode, as only
// they are drawn as sparklines. Rolling mode keeps every dimension per
// bucket so it can subtract what expires.
const SERIES_DIMS = new Set(["sev", "cat"]);

// In unlimited mode a dimension (DNS names, SNIs, ...) can gain keys
// forever. Past this many, the lowest counts are dropped, keeping the
// top half. Distinct counts for a pruned dimension are shown with a "+".
const MAX_KEYS = 20000;

const TICK_MS = 1000;

// Event type cards shown before the rest are combined into "Other".
const MAX_TYPE_CARDS = 10;
const OTHER_TYPES = "\u0000other";

// A sensor whose last stats event is older than this is left out of the
// stats row. Suricata logs stats every 8 seconds by default.
const SENSOR_STALE_MS = 2 * 60 * 1000;
const TOP_N = 10;
const MAX_FEED = 500;
const MAX_RECENT_ALERTS = 40;

// Items first seen within this long are badged as new in the top lists.
const NEW_BADGE_MS = 5 * 60 * 1000;

// Distinct values loaded from the datastore to decide what is new.
const BASELINE_RANGE = "24h";
const BASELINE_SIZE = 10000;

type NoveltyKind = "dns" | "sni" | "http" | "sig";

// First Seen also lists addresses, by which side of the event they were
// on, and Ethernet (MAC) addresses when Suricata logs them. These have
// no history check.
type FeedKind = NoveltyKind | "src" | "dest" | "smac" | "dmac";

const FEED_LABELS: Record<FeedKind, string> = {
  dns: "DNS",
  sni: "SNI",
  http: "HTTP",
  sig: "SIG",
  src: "SRC",
  dest: "DEST",
  smac: "SRC MAC",
  dmac: "DEST MAC",
};

// Search field for each kind; IP addresses link to the Address dashboard
// instead.
const FEED_FIELDS: Record<FeedKind, string> = {
  dns: "dns.rrname",
  sni: "tls.sni",
  http: "http.hostname",
  sig: "alert.signature_id",
  src: "src_ip",
  dest: "dest_ip",
  smac: "ether.src_mac",
  dmac: "ether.dest_mac",
};

const NOVELTY_KINDS: NoveltyKind[] = ["dns", "sni", "http", "sig"];
const IP_KINDS: FeedKind[] = ["src", "dest"];
const ETHER_KINDS: FeedKind[] = ["smac", "dmac"];
const FEED_KINDS: FeedKind[] = [...NOVELTY_KINDS, ...IP_KINDS, ...ETHER_KINDS];

// First Seen is split over three cards so one busy kind doesn't scroll
// the others away.
type FeedGroup = "names" | "ip" | "mac";

function feedGroup(kind: FeedKind): FeedGroup {
  if (kind === "src" || kind === "dest") return "ip";
  if (kind === "smac" || kind === "dmac") return "mac";
  return "names";
}

const BASELINES: { kind: NoveltyKind; field: string; q: string }[] = [
  { kind: "dns", field: "dns.rrname", q: "event_type:dns" },
  { kind: "sni", field: "tls.sni", q: "event_type:tls" },
  { kind: "sni", field: "quic.sni", q: "event_type:quic" },
  { kind: "http", field: "http.hostname", q: "event_type:http" },
  { kind: "sig", field: "alert.signature_id", q: "event_type:alert" },
];

const SEVERITY_COLORS: Record<number, string> = {
  1: "var(--bs-danger)",
  2: "var(--bs-warning)",
  3: "var(--bs-info)",
};

// ---------------------------------------------------------------------
// Rolling window counters.

type Counts = Map<string, Map<string, number>>;

interface Bucket {
  start: number;
  counts: Counts;
}

function bump(counts: Counts, dim: string, key: string, n: number) {
  let m = counts.get(dim);
  if (!m) {
    m = new Map();
    counts.set(dim, m);
  }
  const v = (m.get(key) || 0) + n;
  if (v <= 0) {
    m.delete(key);
  } else {
    m.set(key, v);
  }
}

class Window {
  buckets: Bucket[] = [];
  totals: Counts = new Map();
  bucketMs: number;
  readonly rolling: boolean;
  // Dimensions that have had low counts dropped.
  pruned = new Set<string>();

  constructor(secs: number, now: number) {
    this.rolling = secs > 0;
    this.bucketMs = this.rolling
      ? (secs * 1000) / BUCKETS
      : UNLIMITED_BUCKET_MS;
    this.buckets.push({ start: now, counts: new Map() });
  }

  // Add buckets up to now, then expire (rolling) or merge (unlimited)
  // so there are at most BUCKETS.
  advance(now: number) {
    let last = this.buckets[this.buckets.length - 1];
    while (last.start + this.bucketMs <= now) {
      last = { start: last.start + this.bucketMs, counts: new Map() };
      this.buckets.push(last);
    }
    if (this.rolling) {
      while (this.buckets.length > BUCKETS) {
        const old = this.buckets.shift()!;
        for (const [dim, m] of old.counts) {
          for (const [key, n] of m) {
            bump(this.totals, dim, key, -n);
          }
        }
      }
    } else {
      while (this.buckets.length > BUCKETS) {
        this.mergeBuckets();
      }
      this.prune();
    }
  }

  private mergeBuckets() {
    const merged: Bucket[] = [];
    for (let i = 0; i < this.buckets.length; i += 2) {
      const a = this.buckets[i];
      const b = this.buckets[i + 1];
      if (b) {
        for (const [dim, m] of b.counts) {
          for (const [key, n] of m) {
            bump(a.counts, dim, key, n);
          }
        }
      }
      merged.push(a);
    }
    this.buckets = merged;
    this.bucketMs *= 2;
  }

  private prune() {
    for (const [dim, m] of this.totals) {
      if (m.size <= MAX_KEYS) {
        continue;
      }
      const keep = [...m.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, MAX_KEYS / 2);
      this.totals.set(dim, new Map(keep));
      this.pruned.add(dim);
    }
  }

  add(dim: string, key: string, n = 1) {
    if (this.rolling || SERIES_DIMS.has(dim)) {
      const bucket = this.buckets[this.buckets.length - 1];
      bump(bucket.counts, dim, key, n);
    }
    bump(this.totals, dim, key, n);
  }

  total(dim: string, key = ""): number {
    return this.totals.get(dim)?.get(key) || 0;
  }

  distinct(dim: string): string {
    const n = this.totals.get(dim)?.size || 0;
    return this.pruned.has(dim) ? `${fmtCount(n)}+` : fmtCount(n);
  }

  sum(dim: string): number {
    let total = 0;
    for (const n of this.totals.get(dim)?.values() || []) {
      total += n;
    }
    return total;
  }

  // Per-bucket values for a dimension/key, padded to BUCKETS.
  series(dim: string, key = ""): number[] {
    const values = this.buckets.map((b) => b.counts.get(dim)?.get(key) || 0);
    return Array(Math.max(0, BUCKETS - values.length))
      .fill(0)
      .concat(values);
  }

  top(dim: string, n: number): [string, number][] {
    const m = this.totals.get(dim);
    if (!m) {
      return [];
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, n);
  }
}

// One Window per range, all fed with every event, so switching ranges
// shows counts straight away instead of starting over.
class Ranges {
  private windows: Map<string, Window>;

  constructor(now: number) {
    this.windows = new Map(
      WINDOWS.map((w) => [w.label, new Window(w.secs, now)]),
    );
  }

  get(label: string): Window {
    return this.windows.get(label)!;
  }

  add(dim: string, key: string, n = 1) {
    for (const w of this.windows.values()) {
      w.add(dim, key, n);
    }
  }

  advance(now: number) {
    for (const w of this.windows.values()) {
      w.advance(now);
    }
  }
}

// ---------------------------------------------------------------------
// View models, rebuilt once per tick.

interface TopRow {
  key: string;
  count: number;
  delta: number;
  isNew: boolean;
}

interface FeedItem {
  seq: number;
  at: number;
  kind: FeedKind;
  value: string;
  label: string;
  eventType: string;
  src?: string;
  dest?: string;
}

type NovelItem = FeedItem & { kind: NoveltyKind };

interface RecentAlert {
  seq: number;
  at: string;
  severity: number;
  sid: number;
  signature: string;
  src: string;
  dest: string;
  proto: string;
}

// Latest values from one sensor's stats events. Rates are over the
// interval between its last two stats events.
interface SensorStats {
  host: string;
  at: number;
  pps: number | null;
  bps: number | null;
  // Drop percentage over the last interval.
  dropPct: number | null;
  flows: number | null;
  flowMem: number | null;
}

// Stats from sensors that have reported recently, combined.
interface SensorSummary {
  count: number;
  pps: number | null;
  bps: number | null;
  // Over the selected range.
  packets: number;
  bytes: number;
  drops: number;
  dropPct: number | null;
  // Highest drop percentage over the last interval, with more than one
  // sensor.
  worst: { host: string; dropPct: number } | null;
  flows: number | null;
  flowMem: number | null;
}

interface View {
  eventsInWindow: number;
  eventTypes: [string, number][];
  severities: { sev: number; series: number[]; total: number }[];
  alertTotal: number;
  alertSrcs: string;
  alertDests: string;
  categories: { key: string; count: number; series: number[] }[];
  alertSources: TopRow[];
  alertTargets: TopRow[];
  // Ethernet addresses, only when events carry them.
  etherSeen: boolean;
  srcMacs: TopRow[];
  destMacs: TopRow[];
  srcMacDistinct: string;
  destMacDistinct: string;
  dnsNames: TopRow[];
  snis: TopRow[];
  httpHosts: TopRow[];
  newCounts: Record<NoveltyKind, number>;
  firstCounts: Record<FeedKind, number>;
  sensor: SensorSummary | null;
}

function emptyView(): View {
  return {
    eventsInWindow: 0,
    eventTypes: [],
    severities: [],
    alertTotal: 0,
    alertSrcs: "0",
    alertDests: "0",
    categories: [],
    alertSources: [],
    alertTargets: [],
    etherSeen: false,
    srcMacs: [],
    destMacs: [],
    srcMacDistinct: "0",
    destMacDistinct: "0",
    dnsNames: [],
    snis: [],
    httpHosts: [],
    newCounts: { dns: 0, sni: 0, http: 0, sig: 0 },
    firstCounts: {
      dns: 0,
      sni: 0,
      http: 0,
      sig: 0,
      src: 0,
      dest: 0,
      smac: 0,
      dmac: 0,
    },
    sensor: null,
  };
}

// ---------------------------------------------------------------------
// Formatting.

function fmtCount(n: number): string {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e4) return `${(n / 1e3).toFixed(1)}k`;
  return n.toLocaleString();
}

function fmtBits(bps: number): string {
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(2)} Gb/s`;
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} Mb/s`;
  if (bps >= 1e3) return `${(bps / 1e3).toFixed(0)} Kb/s`;
  return `${Math.round(bps)} b/s`;
}

function fmtBytes(n: number): string {
  if (n >= 2 ** 30) return `${(n / 2 ** 30).toFixed(1)} GiB`;
  if (n >= 2 ** 20) return `${(n / 2 ** 20).toFixed(1)} MiB`;
  if (n >= 2 ** 10) return `${(n / 2 ** 10).toFixed(0)} KiB`;
  return `${n} B`;
}

function fmtPct(n: number): string {
  if (n === 0) return "0%";
  if (n < 0.01) return "<0.01%";
  if (n < 1) return `${n.toFixed(2)}%`;
  return `${n.toFixed(1)}%`;
}

// An address and port for display, with IPv6 compressed and bracketed
// when there is a port, as in "[fe80::1]:53".
function fmtEndpoint(ip: any, port: any): string {
  if (typeof ip !== "string") return "";
  const addr = compressIpv6(ip);
  if (!port) return addr;
  return addr.includes(":") ? `[${addr}]:${port}` : `${addr}:${port}`;
}

function fmtTime(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour12: false });
}

function eventTime(timestamp: any): string {
  if (typeof timestamp !== "string") return "";
  // "2026-11-05T23:33:12.591512-0600" -> "23:33:12"
  const t = timestamp.indexOf("T");
  return t >= 0 ? timestamp.substring(t + 1, t + 9) : timestamp;
}

// ---------------------------------------------------------------------
// Small presentational pieces. Links on this page open in a new tab, so
// following one doesn't lose the counts collected so far.

function Sparkline(props: {
  values: number[];
  color: string;
  height?: number;
  max?: number;
}) {
  const height = () => props.height ?? 28;
  const path = () => {
    const values = props.values;
    if (values.length === 0) return "";
    const max = props.max ?? Math.max(1, ...values);
    const h = height();
    const step = values.length > 1 ? 100 / (values.length - 1) : 100;
    let d = `M0,${h}`;
    values.forEach((v, i) => {
      d += ` L${(i * step).toFixed(2)},${(h - (v / max) * (h - 2)).toFixed(2)}`;
    });
    return d + ` L100,${h} Z`;
  };
  return (
    <svg
      class="topview-spark"
      viewBox={`0 0 100 ${height()}`}
      preserveAspectRatio="none"
      style={{ height: `${height()}px` }}
      aria-hidden="true"
    >
      <line
        x1="0"
        x2="100"
        y1={height() - 0.5}
        y2={height() - 0.5}
        stroke="var(--bs-border-color)"
        stroke-width="1"
        vector-effect="non-scaling-stroke"
      />
      <path d={path()} fill={props.color} fill-opacity="0.55" />
    </svg>
  );
}

// Card header markup follows CountValueDataTable so Top lines up with
// the other dashboards.
function Panel(props: {
  title: string;
  window?: string;
  extra?: JSX.Element;
  tooltip?: string;
  // Card body holds a full width table, like CountValueDataTable.
  table?: boolean;
  // Stretch to the height of the row, to line up with a neighbour.
  fill?: boolean;
  children: JSX.Element;
}) {
  let tooltipRef: HTMLSpanElement | undefined;

  onMount(() => {
    if (tooltipRef && props.tooltip) {
      new Tooltip(tooltipRef, { trigger: "hover" });
    }
  });

  return (
    <div
      classList={{
        card: true,
        "topview-panel": true,
        "h-100": !props.table || props.fill,
        "app-count-value-data-table": props.table,
      }}
    >
      <div class="card-header d-flex">
        <div class="d-flex align-items-center">
          {props.title}
          <Show when={props.tooltip}>
            <span
              ref={tooltipRef}
              class="ms-1"
              data-bs-toggle="tooltip"
              data-bs-placement="top"
              title={props.tooltip}
            >
              <BiInfoCircle class="text-info" />
            </span>
          </Show>
        </div>
        <div class="ms-auto d-flex align-items-center gap-2 small text-muted">
          {props.extra}
          <Show when={props.window}>
            <span class="fst-italic">{props.window}</span>
          </Show>
        </div>
      </div>
      <div
        classList={{
          "card-body": true,
          "p-0": props.table,
          "p-2": !props.table,
        }}
      >
        {props.children}
      </div>
    </div>
  );
}

// Event types in a stable order, the same as the Events page selector,
// so the stat cards don't jump around as counts change.
const EVENT_TYPE_ORDER = new Map(
  EVENT_TYPES.map((et, index) => [et.eventType, index]),
);

function sortEventTypes(types: [string, number][]): [string, number][] {
  return [...types].sort(
    ([a], [b]) =>
      (EVENT_TYPE_ORDER.get(a) ?? EVENT_TYPES.length) -
        (EVENT_TYPE_ORDER.get(b) ?? EVENT_TYPES.length) || a.localeCompare(b),
  );
}

interface TrendRow {
  key: string;
  label: JSX.Element;
  count: number;
  series: number[];
  color: string;
}

// A count table with a sparkline per row, sharing one scale so rows can
// be compared.
function TrendTable(props: { label: string; rows: TrendRow[]; total: number }) {
  const max = () => Math.max(1, ...props.rows.flatMap((row) => row.series));
  return (
    <Show
      when={props.rows.length > 0}
      fallback={<div class="card-body">No data</div>}
    >
      <table class="table">
        <thead>
          <tr>
            <th class="app-count-col">#</th>
            <th>{props.label}</th>
            <th class="topview-trend-col">Trend</th>
            <th class="text-end">%</th>
          </tr>
        </thead>
        <tbody>
          <For each={props.rows}>
            {(row) => (
              <tr>
                <td class="app-count-col">{row.count.toLocaleString()}</td>
                <td class="app-break-anywhere">{row.label}</td>
                <td class="topview-trend-col align-middle">
                  <Sparkline
                    values={row.series}
                    color={row.color}
                    height={20}
                    max={max()}
                  />
                </td>
                <td class="text-end text-muted">
                  {props.total > 0
                    ? ((row.count / props.total) * 100).toFixed(0)
                    : 0}
                  %
                </td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </Show>
  );
}

function newKindSets(): Record<NoveltyKind, Set<string>> {
  return { dns: new Set(), sni: new Set(), http: new Set(), sig: new Set() };
}

function newFeedSets(): Record<FeedKind, Set<string>> {
  return {
    ...newKindSets(),
    src: new Set(),
    dest: new Set(),
    smac: new Set(),
    dmac: new Set(),
  };
}

function KindCounts<K extends FeedKind>(props: {
  kinds: K[];
  counts: Record<K, number>;
}) {
  return (
    <span>
      <For each={props.kinds}>
        {(kind) => (
          <span class="ms-2">
            {FEED_LABELS[kind]} {props.counts[kind]}
          </span>
        )}
      </For>
    </span>
  );
}

function AddressLink(props: { address?: string }) {
  return (
    <Show when={props.address}>
      <A target="_blank" href={`/dashboards/address/${props.address}`}>
        {compressIpv6(props.address!)}
      </A>
    </Show>
  );
}

// A newest-first list of first sightings. The last columns give context:
// "flow" shows both addresses of the event, "peer" the address on the
// other side (for an IP address), and "ip" the IP address on the same
// side (for a MAC address).
function FeedTable(props: {
  items: FeedItem[];
  empty: string;
  valueLabel?: string;
  context?: "flow" | "peer" | "ip";
}) {
  const context = () => props.context ?? "flow";
  const isSrc = (item: FeedItem) => item.kind === "src" || item.kind === "smac";
  return (
    <Show
      when={props.items.length > 0}
      fallback={<div class="card-body">{props.empty}</div>}
    >
      <div class="topview-scroll">
        <table class="table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Type</th>
              <th>{props.valueLabel ?? "Value"}</th>
              <th>Event</th>
              <Show when={context() === "flow"}>
                <th>Source</th>
                <th>Destination</th>
              </Show>
              <Show when={context() === "peer"}>
                <th>Peer</th>
              </Show>
              <Show when={context() === "ip"}>
                <th>IP</th>
              </Show>
            </tr>
          </thead>
          <tbody>
            <For each={props.items}>
              {(item) => (
                <tr
                  classList={{
                    "topview-fresh": Date.now() - item.at < 10000,
                  }}
                >
                  <td class="text-nowrap">{fmtTime(item.at)}</td>
                  <td>
                    <span
                      class={`badge topview-kind topview-kind-${item.kind}`}
                    >
                      {FEED_LABELS[item.kind]}
                    </span>
                  </td>
                  <td class="app-break-anywhere">
                    <Show
                      when={item.kind === "src" || item.kind === "dest"}
                      fallback={
                        <SearchLink
                          target="_blank"
                          field={FEED_FIELDS[item.kind]}
                          value={
                            item.kind === "sig"
                              ? Number(item.value)
                              : item.value
                          }
                        >
                          {item.label}
                        </SearchLink>
                      }
                    >
                      <AddressLink address={item.value} />
                    </Show>
                  </td>
                  <td class="text-nowrap">
                    {eventNameFromType(item.eventType) || item.eventType}
                  </td>
                  <Show when={context() === "flow"}>
                    <td class="text-nowrap">
                      <AddressLink address={item.src} />
                    </td>
                    <td class="text-nowrap">
                      <AddressLink address={item.dest} />
                    </td>
                  </Show>
                  <Show when={context() === "peer"}>
                    <td class="app-break-anywhere">
                      <AddressLink
                        address={isSrc(item) ? item.dest : item.src}
                      />
                    </td>
                  </Show>
                  <Show when={context() === "ip"}>
                    <td class="app-break-anywhere">
                      <AddressLink
                        address={isSrc(item) ? item.src : item.dest}
                      />
                    </td>
                  </Show>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    </Show>
  );
}

function TopList(props: {
  rows: TopRow[];
  label: string;
  field?: string;
  address?: boolean;
}) {
  return (
    <Show
      when={props.rows.length > 0}
      fallback={<div class="card-body">No data</div>}
    >
      <table class="table">
        <thead>
          <tr>
            <th class="app-count-col">#</th>
            <th>{props.label}</th>
          </tr>
        </thead>
        <tbody>
          <For each={props.rows}>
            {(row) => (
              <tr>
                <td class="app-count-col text-nowrap">
                  {row.count.toLocaleString()}
                  <Show when={row.delta > 0}>
                    <span class="topview-delta ms-1">▲{row.delta}</span>
                  </Show>
                </td>
                <td class="app-break-anywhere">
                  <Show
                    when={props.address}
                    fallback={
                      <SearchLink
                        target="_blank"
                        field={props.field}
                        value={row.key}
                      >
                        {row.key}
                      </SearchLink>
                    }
                  >
                    <A target="_blank" href={`/dashboards/address/${row.key}`}>
                      {compressIpv6(row.key)}
                    </A>
                  </Show>
                  <Show when={row.isNew}>
                    <span class="badge topview-badge ms-2">new</span>
                  </Show>
                </td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </Show>
  );
}

// ---------------------------------------------------------------------

export function TopView() {
  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    window?: string;
  }>();

  const windowSpec = () =>
    WINDOWS.find((w) => w.label === searchParams.window) ??
    WINDOWS.find((w) => w.label === DEFAULT_WINDOW)!;

  const [view, setView] = createSignal<View>(emptyView());
  const [feed, setFeed] = createSignal<FeedItem[]>([]);
  const [firstFeed, setFirstFeed] = createSignal<Record<FeedGroup, FeedItem[]>>(
    { names: [], ip: [], mac: [] },
  );
  const [recentAlerts, setRecentAlerts] = createSignal<RecentAlert[]>([]);
  const [received, setReceived] = createSignal(0);
  const [rate, setRate] = createSignal(0);
  const [dropped, setDropped] = createSignal(0);
  const [connection, setConnection] = createSignal<
    "connecting" | "open" | "reconnecting" | "closed"
  >("connecting");
  const [baseline, setBaseline] = createSignal<"loading" | "ready" | "failed">(
    "loading",
  );
  const [baselineSize, setBaselineSize] = createSignal(0);
  const [paused, setPaused] = createSignal(false);
  // Set once any event carries Ethernet addresses (an opt-in Suricata
  // setting), to show the MAC cards. Cleared by reset.
  let etherSeen = false;
  const [startedAt, setStartedAt] = createSignal(Date.now());

  let ranges = new Ranges(Date.now());
  // The window for the selected range.
  const win = () => ranges.get(windowSpec().label);
  let eventSource: EventSource | null = null;
  let tickTimer: number | undefined;
  let seq = 0;
  let receivedCount = 0;
  let lastReceived = 0;
  let lastTickAt = performance.now();

  // Previous tick's counts, to show what moved since the last update.
  let prevCounts = new Map<string, number>();

  // Newly seen, compared against history. `known` holds the baseline
  // plus everything seen since the page opened; `newAt` holds when each
  // value missing from the baseline was first seen, for the "new" badges.
  const known = newKindSets();
  const newAt = new Map<string, number>();
  // Candidates held until the baseline has loaded.
  let pendingNovel: NovelItem[] = [];
  let feedItems: FeedItem[] = [];
  // First sightings on this page, with no history check. Cleared by
  // reset so a fresh start shows everything again.
  let sessionSeen = newFeedSets();
  let firstItems: Record<FeedGroup, FeedItem[]> = {
    names: [],
    ip: [],
    mac: [],
  };
  let alerts: RecentAlert[] = [];
  const sensors = new Map<
    string,
    {
      prev?: { uptime: number; pkts: number; drops: number; bytes: number };
      stats?: SensorStats;
    }
  >();
  // First sighting on this page, for First Seen. No history check.
  function noticedFirst(
    kind: FeedKind,
    value: string,
    label: string,
    event: any,
  ) {
    if (!value || sessionSeen[kind].has(value)) return;
    sessionSeen[kind].add(value);
    const items = firstItems[feedGroup(kind)];
    items.unshift({
      seq: ++seq,
      at: Date.now(),
      kind,
      value,
      label,
      eventType: event.event_type || "",
      src: event.src_ip,
      dest: event.dest_ip,
    });
    if (items.length > MAX_FEED) {
      items.length = MAX_FEED;
    }
    ranges.add("first", kind);
  }

  // A name or signature: a first sighting, and new if missing from the
  // history baseline.
  // Most events log a single src_mac and dest_mac; flow events log
  // src_macs and dest_macs arrays.
  function onEther(event: any) {
    const ether = event.ether;
    const macs = (one: any, many: any): string[] =>
      typeof one === "string"
        ? [one]
        : Array.isArray(many)
          ? many.filter((m: any) => typeof m === "string")
          : [];
    const src = macs(ether.src_mac, ether.src_macs);
    const dest = macs(ether.dest_mac, ether.dest_macs);
    if (src.length === 0 && dest.length === 0) {
      return;
    }
    etherSeen = true;
    for (const mac of src) {
      ranges.add("smac", mac);
      noticedFirst("smac", mac, mac, event);
    }
    for (const mac of dest) {
      ranges.add("dmac", mac);
      noticedFirst("dmac", mac, mac, event);
    }
  }

  function noticed(
    kind: NoveltyKind,
    value: string,
    label: string,
    event: any,
  ) {
    if (!value) return;
    noticedFirst(kind, value, label, event);
    const item: NovelItem = {
      seq: 0,
      at: Date.now(),
      kind,
      value,
      label,
      eventType: event.event_type || "",
      src: event.src_ip,
      dest: event.dest_ip,
    };
    if (baseline() === "loading") {
      pendingNovel.push(item);
      return;
    }
    checkNovel(item);
  }

  function checkNovel(item: NovelItem) {
    const set = known[item.kind];
    if (set.has(item.value)) return;
    set.add(item.value);
    newAt.set(`${item.kind}:${item.value}`, item.at);
    item.seq = ++seq;
    feedItems.unshift(item);
    if (feedItems.length > MAX_FEED) {
      feedItems.length = MAX_FEED;
    }
    ranges.add("new", item.kind);
  }

  function isNew(kind: NoveltyKind, value: string, now: number): boolean {
    const at = newAt.get(`${kind}:${value}`);
    return at !== undefined && now - at < NEW_BADGE_MS;
  }

  function onEvent(event: any) {
    const type: string = event.event_type || "unknown";
    ranges.add("type", type);
    ranges.add("events", "");
    if (typeof event.src_ip === "string") {
      noticedFirst("src", event.src_ip, event.src_ip, event);
    }
    if (typeof event.dest_ip === "string") {
      noticedFirst("dest", event.dest_ip, event.dest_ip, event);
    }
    if (event.ether && typeof event.ether === "object") {
      onEther(event);
    }

    switch (type) {
      case "alert": {
        const alert = event.alert || {};
        const sev = Math.min(Math.max(Number(alert.severity) || 3, 1), 3);
        ranges.add("sev", String(sev));
        ranges.add("cat", alert.category || "(none)");
        if (event.src_ip) ranges.add("asrc", event.src_ip);
        if (event.dest_ip) ranges.add("adst", event.dest_ip);
        alerts.unshift({
          seq: ++seq,
          at: eventTime(event.timestamp),
          severity: sev,
          sid: alert.signature_id,
          signature: alert.signature || "",
          src: fmtEndpoint(event.src_ip, event.src_port),
          dest: fmtEndpoint(event.dest_ip, event.dest_port),
          proto: event.app_proto || event.proto || "",
        });
        if (alerts.length > MAX_RECENT_ALERTS) {
          alerts.length = MAX_RECENT_ALERTS;
        }
        if (alert.signature_id !== undefined) {
          noticed("sig", String(alert.signature_id), alert.signature, event);
        }
        break;
      }
      case "dns": {
        const dns = event.dns || {};
        const kind = dns.type;
        const rrname: string | undefined =
          dns.rrname ?? dns.queries?.[0]?.rrname;
        if ((kind === "query" || kind === "request") && rrname) {
          ranges.add("dnsname", rrname);
          noticed("dns", rrname, rrname, event);
        }
        break;
      }
      case "tls":
      case "quic": {
        const proto = event[type] || {};
        if (proto.sni) {
          ranges.add("sni", proto.sni);
          noticed("sni", proto.sni, proto.sni, event);
        }
        break;
      }
      case "http": {
        const hostname = event.http?.hostname;
        if (hostname) {
          ranges.add("http", hostname);
          noticed("http", hostname, hostname, event);
        }
        break;
      }
      case "stats":
        onStats(event);
        break;
    }
  }

  // Sensors are told apart by the event "host" field, the same field the
  // sensor filter uses. Counters are cumulative, so rates come from the
  // difference between consecutive stats events; a lower uptime means
  // Suricata restarted and the counters started over.
  function onStats(event: any) {
    const s = event.stats;
    if (!s || typeof s !== "object") return;
    const host: string = event.host || "";
    const entry = sensors.get(host) ?? {};
    sensors.set(host, entry);

    const num = (v: any): number | null =>
      typeof v === "number" && Number.isFinite(v) ? v : null;
    const uptime = num(s.uptime) ?? 0;
    const pkts = num(s.capture?.kernel_packets) ?? num(s.decoder?.pkts) ?? 0;
    const drops = num(s.capture?.kernel_drops) ?? 0;
    const bytes = num(s.decoder?.bytes) ?? 0;

    const prev = entry.prev;
    entry.prev = { uptime, pkts, drops, bytes };
    let bps: number | null = null;
    let pps: number | null = null;
    let dropPct: number | null = null;
    if (prev && uptime > prev.uptime && pkts >= prev.pkts) {
      const dt = uptime - prev.uptime;
      const dp = pkts - prev.pkts;
      const dd = Math.max(0, drops - prev.drops);
      pps = dp / dt;
      dropPct = dp > 0 ? (dd / dp) * 100 : 0;
      // Counted into the window so drops follow the selected range.
      ranges.add("capture", "pkts", dp);
      ranges.add("capture", "drops", dd);
      if (bytes >= prev.bytes) {
        bps = ((bytes - prev.bytes) * 8) / dt;
        ranges.add("capture", "bytes", bytes - prev.bytes);
      }
    }

    entry.stats = {
      host,
      at: Date.now(),
      pps,
      bps,
      dropPct,
      flows: num(s.flow?.active),
      flowMem: num(s.flow?.memuse),
    };
  }

  function summarizeSensors(now: number): SensorSummary | null {
    const fresh = [...sensors.values()]
      .map((entry) => entry.stats)
      .filter(
        (stats): stats is SensorStats =>
          !!stats && now - stats.at < SENSOR_STALE_MS,
      );
    if (fresh.length === 0) {
      return null;
    }
    // Sum a field over the sensors that report it, null if none do.
    const sum = (field: keyof SensorStats): number | null => {
      const values = fresh
        .map((stats) => stats[field])
        .filter((v): v is number => typeof v === "number");
      return values.length > 0 ? values.reduce((a, b) => a + b, 0) : null;
    };
    const pkts = win().total("capture", "pkts");
    const drops = win().total("capture", "drops");
    let worst: SensorSummary["worst"] = null;
    if (fresh.length > 1) {
      for (const stats of fresh) {
        if (
          stats.dropPct !== null &&
          stats.dropPct > 0 &&
          (worst === null || stats.dropPct > worst.dropPct)
        ) {
          worst = { host: stats.host || "(no name)", dropPct: stats.dropPct };
        }
      }
    }
    return {
      count: fresh.length,
      pps: sum("pps"),
      bps: sum("bps"),
      packets: pkts,
      bytes: win().total("capture", "bytes"),
      drops,
      dropPct: pkts > 0 ? (drops / pkts) * 100 : null,
      worst,
      flows: sum("flows"),
      flowMem: sum("flowMem"),
    };
  }

  function topRows(dim: string, kind: NoveltyKind | null, now: number) {
    return win()
      .top(dim, TOP_N)
      .map(([key, count]) => {
        const id = `${dim}:${key}`;
        const prev = prevCounts.get(id);
        return {
          key,
          count,
          delta: prev === undefined ? 0 : Math.max(0, count - prev),
          isNew: kind ? isNew(kind, key, now) : false,
        };
      });
  }

  function rememberCounts(dims: string[]) {
    const next = new Map<string, number>();
    for (const dim of dims) {
      for (const [key, count] of win().totals.get(dim) || []) {
        next.set(`${dim}:${key}`, count);
      }
    }
    prevCounts = next;
  }

  // Once a second: sample the event rate, then render. The rate is only
  // sampled here so an extra render, such as on a range change, doesn't
  // measure it over a fraction of a second.
  function tick() {
    const at = performance.now();
    const elapsed = Math.max(0.001, (at - lastTickAt) / 1000);
    setRate(Math.round((receivedCount - lastReceived) / elapsed));
    lastReceived = receivedCount;
    lastTickAt = at;
    render();
  }

  function render() {
    const now = Date.now();
    ranges.advance(now);
    setReceived(receivedCount);

    if (paused() || document.visibilityState === "hidden") {
      return;
    }

    const dnsNames = topRows("dnsname", "dns", now);
    const snis = topRows("sni", "sni", now);
    const httpHosts = topRows("http", "http", now);
    const alertSources = topRows("asrc", null, now);
    const alertTargets = topRows("adst", null, now);
    const srcMacs = topRows("smac", null, now);
    const destMacs = topRows("dmac", null, now);
    rememberCounts(["dnsname", "sni", "http", "asrc", "adst", "smac", "dmac"]);

    setView({
      sensor: summarizeSensors(now),
      eventsInWindow: win().total("events"),
      eventTypes: sortEventTypes(win().top("type", 1000)),
      severities: [1, 2, 3].map((sev) => ({
        sev,
        series: win().series("sev", String(sev)),
        total: win().total("sev", String(sev)),
      })),
      alertTotal: win().sum("sev"),
      alertSrcs: win().distinct("asrc"),
      alertDests: win().distinct("adst"),
      categories: win()
        .top("cat", 6)
        .map(([key, count]) => ({
          key,
          count,
          series: win().series("cat", key),
        })),
      alertSources,
      alertTargets,
      etherSeen,
      srcMacs,
      destMacs,
      srcMacDistinct: win().distinct("smac"),
      destMacDistinct: win().distinct("dmac"),
      dnsNames,
      snis,
      httpHosts,
      newCounts: {
        dns: win().total("new", "dns"),
        sni: win().total("new", "sni"),
        http: win().total("new", "http"),
        sig: win().total("new", "sig"),
      },
      firstCounts: Object.fromEntries(
        FEED_KINDS.map((kind) => [kind, win().total("first", kind)]),
      ) as Record<FeedKind, number>,
    });
    setFeed([...feedItems]);
    setFirstFeed({
      names: [...firstItems.names],
      ip: [...firstItems.ip],
      mac: [...firstItems.mac],
    });
    setRecentAlerts([...alerts]);
  }

  async function loadBaseline() {
    setBaseline("loading");
    try {
      const results = await Promise.all(
        BASELINES.map((b) =>
          API.fetchAgg({
            field: b.field,
            q: b.q,
            time_range: BASELINE_RANGE,
            size: BASELINE_SIZE,
          }).then((response) => ({ kind: b.kind, rows: response.rows })),
        ),
      );
      let size = 0;
      for (const { kind, rows } of results) {
        for (const row of rows) {
          if (row.key !== null && row.key !== undefined && row.key !== "") {
            known[kind].add(String(row.key));
            size += 1;
          }
        }
      }
      setBaselineSize(size);
      setBaseline("ready");
    } catch (e) {
      console.log(`Top: failed to load baseline: ${e}`);
      // Fall back to "new since this page was opened".
      setBaseline("failed");
    }
    const pending = pendingNovel;
    pendingNovel = [];
    pending.forEach(checkNovel);
  }

  function connect() {
    disconnect();
    setConnection("connecting");
    const params = new URLSearchParams();
    if (searchParams.sensor) {
      params.set("sensor", searchParams.sensor);
    }
    const query = params.toString();
    const es = new EventSource("api/firehose/sse" + (query ? `?${query}` : ""));
    eventSource = es;
    es.onopen = () => setConnection("open");
    es.onerror = () =>
      setConnection(
        es.readyState === EventSource.CLOSED ? "closed" : "reconnecting",
      );
    es.onmessage = (message) => {
      let event: any;
      try {
        event = JSON.parse(message.data);
      } catch (_e) {
        return;
      }
      if (!event || typeof event !== "object") return;
      receivedCount += 1;
      onEvent(event);
    };
    es.addEventListener("gap", (message: MessageEvent) => {
      try {
        const n = Number(JSON.parse(message.data)?.dropped) || 0;
        setDropped((total) => total + n);
      } catch (_e) {}
    });
  }

  function disconnect() {
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
  }

  function reset() {
    const now = Date.now();
    ranges = new Ranges(now);
    setStartedAt(now);
    prevCounts = new Map();
    feedItems = [];
    sessionSeen = newFeedSets();
    etherSeen = false;
    firstItems = { names: [], ip: [], mac: [] };
    alerts = [];
    sensors.clear();
    receivedCount = 0;
    lastReceived = 0;
    setDropped(0);
    render();
  }

  let rangeLabelRef: HTMLLabelElement | undefined;

  onMount(() => {
    if (rangeLabelRef) {
      new Tooltip(rangeLabelRef, { trigger: "hover" });
    }
    loadBaseline();
    tickTimer = window.setInterval(tick, TICK_MS);
  });

  onCleanup(() => {
    disconnect();
    if (tickTimer !== undefined) {
      window.clearInterval(tickTimer);
    }
  });

  createEffect(
    on(
      () => searchParams.sensor,
      () => {
        reset();
        connect();
      },
    ),
  );

  // Switch to another range's counts.
  createEffect(
    on(
      () => windowSpec().label,
      () => {
        // Every range is counted all along, so only the "since last
        // update" deltas need clearing, as they compare across ranges.
        prevCounts = new Map();
        render();
      },
      { defer: true },
    ),
  );

  const status = (): "live" | "paused" | "connecting" | "closed" =>
    paused()
      ? "paused"
      : connection() === "open"
        ? "live"
        : connection() === "closed"
          ? "closed"
          : "connecting";

  // At most MAX_TYPE_CARDS event type cards so they fit on one row; past
  // that the smallest are combined into an "Other" card.
  const eventTypeCards = (): [string, number][] => {
    const types = view().eventTypes;
    if (types.length <= MAX_TYPE_CARDS) {
      return types;
    }
    const keep = new Set(
      [...types]
        .sort((a, b) => b[1] - a[1])
        .slice(0, MAX_TYPE_CARDS - 1)
        .map(([type]) => type),
    );
    const other = types
      .filter(([type]) => !keep.has(type))
      .reduce((total, [, count]) => total + count, 0);
    return [...types.filter(([type]) => keep.has(type)), [OTHER_TYPES, other]];
  };

  // Shown on each card header.
  const windowLabel = () =>
    windowSpec().secs > 0
      ? `last ${windowSpec().label}`
      : `since ${fmtTime(startedAt())}`;

  return (
    <>
      <Top disableRange />
      <style>{TOPVIEW_CSS}</style>
      <Container fluid class="topview pb-3">
        <div
          class="alert alert-warning py-1 px-2 mt-2 mb-0 small text-center"
          role="status"
        >
          This page is under construction and subject to change.
        </div>

        {/* Controls, with the sensor filter on the right. */}
        <div class="row mt-2">
          <div class="col">
            <form
              class="d-flex flex-wrap align-items-center gap-2"
              onsubmit={(e) => e.preventDefault()}
            >
              {/* Doubles as the connection status. */}
              <button
                type="button"
                classList={{
                  btn: true,
                  "btn-success": status() === "live",
                  "btn-warning": status() === "paused",
                  "btn-secondary": status() === "connecting",
                  "btn-danger": status() === "closed",
                }}
                style={"min-width: 7.5em;"}
                onclick={(e) => {
                  setPaused((p) => !p);
                  e.currentTarget.blur();
                }}
                title={
                  paused()
                    ? "Paused, click to resume"
                    : status() === "closed"
                      ? "Disconnected from the server"
                      : "Click to pause updates"
                }
              >
                {
                  {
                    live: "● Live",
                    paused: "Paused",
                    connecting: "Connecting",
                    closed: "Closed",
                  }[status()]
                }
              </button>
              <button
                type="button"
                class="btn btn-secondary"
                onclick={(e) => {
                  reset();
                  e.currentTarget.blur();
                }}
                title="Clear all counts and start over"
              >
                Reset
              </button>
            </form>
          </div>
          <div class="col-auto ms-auto d-flex align-items-center gap-2">
            <div class="d-inline-flex">
              <div class="input-group flex-nowrap">
                <label
                  ref={rangeLabelRef}
                  class="input-group-text"
                  data-bs-toggle="tooltip"
                  data-bs-placement="bottom"
                  title={RANGE_TOOLTIP}
                  style={"cursor: help;"}
                >
                  Range
                </label>
                <select
                  class="form-select"
                  style={"width: auto;"}
                  onchange={(e) => {
                    setSearchParams({ window: e.currentTarget.value });
                    e.currentTarget.blur();
                  }}
                >
                  <For each={WINDOWS}>
                    {(w) => (
                      <option
                        value={w.label}
                        selected={w.label === windowSpec().label}
                      >
                        {w.name}
                      </option>
                    )}
                  </For>
                </select>
              </div>
            </div>
            <div class="d-inline-flex">
              <SensorSelect
                selected={searchParams.sensor}
                onchange={(sensor) => setSearchParams({ sensor })}
              />
            </div>
          </div>
        </div>

        {/* Sensor stats, from stats events. */}
        <div class="row mt-2 g-2">
          <div class="col topview-stat-col">
            <StatCard
              value={`${rate().toLocaleString()}/s`}
              label="Events"
              sub={
                dropped() > 0
                  ? `${received().toLocaleString()} received, ${fmtCount(dropped())} missed`
                  : `${received().toLocaleString()} received`
              }
            />
          </div>
          <div class="col topview-stat-col">
            <StatCard
              value={
                view().sensor ? view().sensor!.packets.toLocaleString() : null
              }
              label="Packets"
              sub={
                view().sensor?.pps != null
                  ? `${Math.round(view().sensor!.pps!).toLocaleString()}/s`
                  : undefined
              }
            />
          </div>
          <div class="col topview-stat-col">
            <StatCard
              value={view().sensor ? fmtBytes(view().sensor!.bytes) : null}
              label="Bytes"
              sub={
                view().sensor?.bps != null
                  ? fmtBits(view().sensor!.bps!)
                  : undefined
              }
            />
          </div>
          <div class="col topview-stat-col">
            <StatCard
              value={
                view().sensor ? view().sensor!.drops.toLocaleString() : null
              }
              valueClass={
                (view().sensor?.dropPct ?? 0) >= 1
                  ? "text-danger"
                  : (view().sensor?.dropPct ?? 0) > 0
                    ? "text-warning"
                    : ""
              }
              label="Drops"
              sub={
                view().sensor?.dropPct != null
                  ? view().sensor!.worst
                    ? `${fmtPct(view().sensor!.dropPct!)}, worst ${view().sensor!.worst!.host}`
                    : fmtPct(view().sensor!.dropPct!)
                  : undefined
              }
            />
          </div>
          <div class="col topview-stat-col">
            <StatCard
              value={
                view().sensor?.flows != null
                  ? view().sensor!.flows!.toLocaleString()
                  : null
              }
              label="Active Flows"
              sub={
                view().sensor?.flowMem != null
                  ? `${fmtBytes(view().sensor!.flowMem!)} memory`
                  : undefined
              }
            />
          </div>
        </div>

        {/* Event type counts, kept to a single row. */}
        <div class="row mt-2 g-2 flex-nowrap">
          <For each={eventTypeCards()}>
            {([type, count]) => (
              <div class="col topview-stat-col">
                <StatCard
                  value={count.toLocaleString()}
                  label={
                    type === OTHER_TYPES
                      ? "Other"
                      : eventNameFromType(type) || type
                  }
                  sub={
                    view().eventsInWindow > 0
                      ? `${((count / view().eventsInWindow) * 100).toFixed(1)}%`
                      : undefined
                  }
                />
              </div>
            )}
          </For>
        </div>

        {/* Alerts by severity and category. */}
        <div class="row mt-2 g-2">
          <div class="col-lg-6">
            <Panel
              title="Alert Severities"
              fill
              window={windowLabel()}
              extra={`${view().alertTotal.toLocaleString()} alerts`}
              table
            >
              <TrendTable
                label="Severity"
                total={view().alertTotal}
                rows={view()
                  .severities.filter((s) => s.total > 0)
                  .map((s) => ({
                    key: String(s.sev),
                    label: (
                      <SearchLink
                        target="_blank"
                        field="alert.severity"
                        value={s.sev}
                      >
                        <span style={{ color: SEVERITY_COLORS[s.sev] }}>
                          Severity {s.sev}
                        </span>
                      </SearchLink>
                    ),
                    count: s.total,
                    series: s.series,
                    color: SEVERITY_COLORS[s.sev],
                  }))}
              />
            </Panel>
          </div>
          <div class="col-lg-6">
            <Panel title="Alert Categories" window={windowLabel()} table fill>
              <TrendTable
                label="Category"
                total={view().alertTotal}
                rows={view().categories.map((c) => ({
                  key: c.key,
                  label: (
                    <SearchLink
                      target="_blank"
                      field="alert.category"
                      value={c.key}
                    >
                      {c.key}
                    </SearchLink>
                  ),
                  count: c.count,
                  series: c.series,
                  color: "var(--bs-primary)",
                }))}
              />
            </Panel>
          </div>
        </div>

        {/* Alert sources and targets, always side by side. */}
        <div class="row mt-2 g-2">
          <div class="col-6">
            <Panel
              title="Alert Sources"
              window={windowLabel()}
              extra={`${view().alertSrcs} distinct`}
              table
            >
              <TopList rows={view().alertSources} label="Address" address />
            </Panel>
          </div>
          <div class="col-6">
            <Panel
              title="Alert Targets"
              window={windowLabel()}
              extra={`${view().alertDests} distinct`}
              table
            >
              <TopList rows={view().alertTargets} label="Address" address />
            </Panel>
          </div>
        </div>

        {/* Ethernet addresses, only once Suricata is logging them. */}
        <Show when={view().etherSeen}>
          <div class="row mt-2 g-2">
            <div class="col-6">
              <Panel
                title="Source MAC Addresses"
                window={windowLabel()}
                extra={`${view().srcMacDistinct} distinct`}
                table
              >
                <TopList
                  rows={view().srcMacs}
                  label="Address"
                  field="ether.src_mac"
                />
              </Panel>
            </div>
            <div class="col-6">
              <Panel
                title="Destination MAC Addresses"
                window={windowLabel()}
                extra={`${view().destMacDistinct} distinct`}
                table
              >
                <TopList
                  rows={view().destMacs}
                  label="Address"
                  field="ether.dest_mac"
                />
              </Panel>
            </div>
          </div>
        </Show>

        {/* DNS and TLS/QUIC names. */}
        <div class="row mt-2 g-2">
          <div class="col-md-6">
            <Panel
              title="DNS Names"
              window={windowLabel()}
              extra={
                <span classList={{ "text-warning": view().newCounts.dns > 0 }}>
                  {view().newCounts.dns} new
                </span>
              }
              table
            >
              <TopList rows={view().dnsNames} label="Name" field="dns.rrname" />
            </Panel>
          </div>
          <div class="col-md-6">
            <Panel
              title="Server Names"
              window={windowLabel()}
              tooltip="TLS and QUIC server names (SNI)."
              extra={
                <span classList={{ "text-warning": view().newCounts.sni > 0 }}>
                  {view().newCounts.sni} new
                </span>
              }
              table
            >
              <TopList
                rows={view().snis}
                label="Server Name (SNI)"
                field="tls.sni"
              />
            </Panel>
          </div>
        </div>

        {/* First seen on this page, no history check: MAC and IP
            addresses side by side, then names and signatures. */}
        <div class="row mt-2 g-2">
          <Show when={view().etherSeen}>
            <div class="col-6">
              <Panel
                title="First Seen MAC Addresses"
                window={windowLabel()}
                tooltip="Each Ethernet address the first time this page sees it, as a source or destination, with the IP address it was using. Reset clears it."
                extra={
                  <KindCounts kinds={ETHER_KINDS} counts={view().firstCounts} />
                }
                table
              >
                <FeedTable
                  items={firstFeed().mac}
                  empty="Waiting for events."
                  valueLabel="Address"
                  context="ip"
                />
              </Panel>
            </div>
          </Show>
          <div
            classList={{
              "col-6": view().etherSeen,
              "col-12": !view().etherSeen,
            }}
          >
            <Panel
              title="First Seen IP Addresses"
              window={windowLabel()}
              tooltip="Each IP address the first time this page sees it, as a source or destination, with the address on the other side. Reset clears it."
              extra={
                <KindCounts kinds={IP_KINDS} counts={view().firstCounts} />
              }
              table
            >
              <FeedTable
                items={firstFeed().ip}
                empty="Waiting for events."
                valueLabel="Address"
                context="peer"
              />
            </Panel>
          </div>
        </div>
        <div class="row mt-2 g-2">
          <div class="col-12">
            <Panel
              title="First Seen Names and Signatures"
              window={windowLabel()}
              tooltip="Every DNS name, TLS/QUIC SNI, HTTP host and alert signature the first time this page sees it, whether or not it has been seen before. Reset clears it, so you can watch what a new app or device connects to."
              extra={
                <KindCounts kinds={NOVELTY_KINDS} counts={view().firstCounts} />
              }
              table
            >
              <FeedTable
                items={firstFeed().names}
                empty="Waiting for events."
              />
            </Panel>
          </div>
        </div>

        {/* Newly seen, compared against history, full width. */}
        <div class="row mt-2 g-2">
          <div class="col-12">
            <Panel
              title="Newly Seen"
              window={windowLabel()}
              tooltip={`DNS names, TLS/QUIC SNIs, HTTP hosts and alert signatures not seen in the last ${BASELINE_RANGE}, according to the datastore. Falls back to "since this page was opened" if the lookup fails.`}
              extra={
                <KindCounts kinds={NOVELTY_KINDS} counts={view().newCounts} />
              }
              table
            >
              <FeedTable
                items={feed()}
                empty="Nothing new yet. DNS names, SNIs, HTTP hosts and alert signatures not seen in the last 24 hours will appear here."
              />
              <div class="small text-muted px-2 py-1 border-top">
                <Show
                  when={baseline() === "ready"}
                  fallback={
                    baseline() === "loading"
                      ? `Loading the last ${BASELINE_RANGE} for comparison…`
                      : "Comparing against events since this page was opened."
                  }
                >
                  Compared against {baselineSize().toLocaleString()} values seen
                  in the last {BASELINE_RANGE}.
                </Show>
              </div>
            </Panel>
          </div>
        </div>

        {/* Recent alerts, full width. */}
        <div class="row mt-2 g-2">
          <div class="col-12">
            <Panel
              title="Recent Alerts"
              extra={
                <A target="_blank" href="/live?event_type=alert">
                  open in Live view →
                </A>
              }
              table
            >
              <Show
                when={recentAlerts().length > 0}
                fallback={<div class="card-body">No data</div>}
              >
                <div class="topview-scroll">
                  <table class="table">
                    <thead>
                      <tr>
                        <th>Time</th>
                        <th>Sev</th>
                        <th>Signature</th>
                        <th>Source</th>
                        <th>Destination</th>
                        <th>Proto</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={recentAlerts()}>
                        {(a) => (
                          <tr>
                            <td class="text-nowrap">{a.at}</td>
                            <td style={{ color: SEVERITY_COLORS[a.severity] }}>
                              {a.severity}
                            </td>
                            <td class="app-break-anywhere">
                              <SearchLink
                                target="_blank"
                                field="alert.signature_id"
                                value={a.sid}
                              >
                                {a.signature}
                              </SearchLink>
                            </td>
                            <td class="text-nowrap">{a.src}</td>
                            <td class="text-nowrap">{a.dest}</td>
                            <td>{a.proto}</td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                </div>
              </Show>
            </Panel>
          </div>
        </div>
      </Container>
    </>
  );
}

const TOPVIEW_CSS = `
.topview .topview-spark { display: block; width: 100%; }
.topview a { text-decoration: none; }
.topview a:hover { text-decoration: underline; }
.topview .topview-trend-col { width: 35%; }
.topview .topview-stat-col { min-width: 0; }
.topview .topview-delta { color: var(--bs-success); font-size: 0.75em; }
.topview .topview-badge { background: var(--bs-warning-bg-subtle); color: var(--bs-warning-text-emphasis); }
.topview .topview-kind { min-width: 3.4em; }
.topview .topview-kind-smac, .topview .topview-kind-dmac { background: var(--bs-tertiary-bg); color: var(--bs-body-color); }
.topview .topview-kind-dns { background: var(--bs-info-bg-subtle); color: var(--bs-info-text-emphasis); }
.topview .topview-kind-sni { background: var(--bs-success-bg-subtle); color: var(--bs-success-text-emphasis); }
.topview .topview-kind-http { background: var(--bs-primary-bg-subtle); color: var(--bs-primary-text-emphasis); }
.topview .topview-kind-src { background: var(--bs-secondary-bg); color: var(--bs-secondary-color); }
.topview .topview-kind-dest { background: var(--bs-warning-bg-subtle); color: var(--bs-warning-text-emphasis); }
.topview .topview-kind-sig { background: var(--bs-danger-bg-subtle); color: var(--bs-danger-text-emphasis); }
.topview .topview-fresh > td { animation: topview-fade 10s ease-out; }
@keyframes topview-fade { from { background-color: var(--bs-warning-bg-subtle); } }
.topview .topview-scroll { max-height: 32rem; overflow-y: auto; }
.topview .topview-scroll thead th { position: sticky; top: 0; z-index: 1; }
`;
