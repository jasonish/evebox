// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// A live, non-aggregated scroller of events as they arrive at the
// server, fed by the server's firehose SSE endpoint. Filtering is done
// server side so the retained rows are the matching rows, even on a
// busy sensor.

import { Top } from "./Top";
import * as API from "./api";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Match,
  on,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { useSearchParams } from "@solidjs/router";
import { Col, Container, Row } from "solid-bootstrap";
import { tinykeys } from "tinykeys";
import { EventWrapper } from "./types";
import { formatAddressWithPort, formatEventDescription } from "./formatters";
import { TimestampCell } from "./components";
import { AlertDescription } from "./Alerts";
import { AppProtoBadge, EVENT_TYPES, eventNameFromType } from "./Events";
import { SensorSelect } from "./common/SensorSelect";

const MAX_ROW_OPTIONS = [100, 250, 500, 1000];
const DEFAULT_MAX_ROWS = 500;

// The event type dropdown is built from the types seen in the datastore
// over this time range, falling back to the static list.
const EVENT_TYPE_LOOKBACK = "24h";

// Incoming events are batched and applied on an interval so a busy sensor
// doesn't trigger a re-render for every single event.
const FLUSH_INTERVAL_MS = 250;

interface LiveEventRow {
  kind: "event";
  seq: number;
  event: EventWrapper;
}

// A marker for events that are known to be missing from the view.
interface LiveGapRow {
  kind: "gap";
  seq: number;
  // Number of events known to be missing, 0 when unknown.
  dropped: number;
  reasons: string[];
  at: string;
}

const REASON_SERVER = "server could not keep up";
const REASON_CLIENT = "browser could not keep up";
const REASON_PAUSED = "discarded while paused";
const REASON_RECONNECT = "connection lost";

function gapMessage(row: LiveGapRow): string {
  const reasons = row.reasons.join(", ");
  if (row.dropped > 0) {
    return `${row.dropped.toLocaleString()} events missing: ${reasons}`;
  }
  return `Events may have been missed: ${reasons}`;
}

type LiveRow = LiveEventRow | LiveGapRow;

type ConnectionState = "connecting" | "open" | "reconnecting" | "closed";

export function Live() {
  const [searchParams, setSearchParams] = useSearchParams<{
    event_type?: string;
    sensor?: string;
    q?: string;
  }>();
  const [rows, setRows] = createSignal<LiveRow[]>([], {
    equals: false,
  });
  const [paused, setPaused] = createSignal(false);
  const [connection, setConnection] =
    createSignal<ConnectionState>("connecting");
  const [received, setReceived] = createSignal(0);
  // Events per second, sampled once a second from the received counter.
  const [rate, setRate] = createSignal(0);
  const [pendingCount, setPendingCount] = createSignal(0);
  // Events known to be missing: dropped by the server, or discarded here
  // while paused.
  const [dropped, setDropped] = createSignal(0);
  const [maxRows, setMaxRows] = createSignal(DEFAULT_MAX_ROWS);
  const [expanded, setExpanded] = createSignal<Set<number>>(new Set(), {
    equals: false,
  });
  const [eventTypes, setEventTypes] =
    createSignal<{ name: string; eventType: string }[]>(EVENT_TYPES);

  let eventSource: EventSource | null = null;
  let seq = 0;
  let incoming: LiveRow[] = [];
  // Number of event rows (not gap markers) in `incoming`.
  let incomingEvents = 0;
  let pausedDropped = 0;
  let pausedReasons = new Set<string>();
  let lostConnection = false;
  let flushTimer: number | undefined;
  let rateTimer: number | undefined;
  let lastReceived = 0;
  let keybindings: (() => void) | null = null;

  const eventRows = createMemo(
    () => rows().filter((row) => row.kind === "event") as LiveEventRow[],
  );

  function nextSeq(): number {
    seq += 1;
    return seq;
  }

  function gapRow(dropped: number, reasons: string[]): LiveGapRow {
    return {
      kind: "gap",
      seq: nextSeq(),
      dropped,
      reasons,
      at: new Date().toLocaleTimeString(),
    };
  }

  // Limit the view to the newest `maxRows` events. Gap markers don't count
  // against the limit so a marker directly below the oldest shown event
  // survives, which is exactly where it says something.
  function trim(list: LiveRow[]): LiveRow[] {
    const max = maxRows();
    const kept: LiveRow[] = [];
    let events = 0;
    for (const row of list) {
      if (row.kind === "event") {
        if (events >= max) {
          break;
        }
        events += 1;
      }
      kept.push(row);
    }
    return kept;
  }

  function flush() {
    if (incoming.length === 0) {
      return;
    }
    if (paused()) {
      setPendingCount(incoming.length);
      return;
    }
    let batch = incoming;
    incoming = [];
    incomingEvents = 0;
    setPendingCount(0);
    if (pausedDropped > 0 || pausedReasons.size > 0) {
      // Oldest first at this point, so the marker lands between the
      // events kept while paused and the rows that were already shown.
      batch = [gapRow(pausedDropped, [...pausedReasons]), ...batch];
      pausedDropped = 0;
      pausedReasons = new Set();
    }
    // Newest first, like the inbox.
    batch.reverse();
    setRows((current) => trim(batch.concat(current)));
  }

  function queue(row: LiveRow) {
    const last = incoming[incoming.length - 1];
    if (row.kind === "gap" && last && last.kind === "gap") {
      // Collapse consecutive gaps into one marker.
      last.dropped += row.dropped;
      for (const reason of row.reasons) {
        if (!last.reasons.includes(reason)) {
          last.reasons.push(reason);
        }
      }
      last.at = row.at;
      return;
    }
    incoming.push(row);
    if (row.kind === "event") {
      incomingEvents += 1;
    }
    // Bound the buffer, keeping the most recent events and counting what
    // was lost so a gap can be shown on the next flush. This is normally
    // only reached while paused; unpaused, the buffer is flushed every
    // FLUSH_INTERVAL_MS. Evicted gap markers are folded into that gap.
    const max = maxRows();
    if (incomingEvents > max) {
      let removedEvents = 0;
      while (incomingEvents > max && incoming.length > 0) {
        const removed = incoming.shift()!;
        if (removed.kind === "event") {
          incomingEvents -= 1;
          removedEvents += 1;
        } else {
          pausedDropped += removed.dropped;
          removed.reasons.forEach((reason) => pausedReasons.add(reason));
        }
      }
      pausedDropped += removedEvents;
      pausedReasons.add(paused() ? REASON_PAUSED : REASON_CLIENT);
      setDropped((n) => n + removedEvents);
    }
    if (paused()) {
      setPendingCount(incoming.length);
    }
  }

  function onMessage(message: MessageEvent) {
    let source: any;
    try {
      source = JSON.parse(message.data);
    } catch (e) {
      console.log(`Failed to decode live event: ${e}`);
      return;
    }
    if (!source || typeof source !== "object") {
      return;
    }
    const event: EventWrapper = {
      _id: "",
      _source: source,
      __private: { selected: false },
    };
    setReceived((n) => n + 1);
    queue({ kind: "event", seq: nextSeq(), event });
  }

  function onGap(message: MessageEvent) {
    let n = 0;
    try {
      n = Number(JSON.parse(message.data)?.dropped) || 0;
    } catch (e) {
      console.log(`Failed to decode gap event: ${e}`);
    }
    setDropped((total) => total + n);
    queue(gapRow(n, [REASON_SERVER]));
  }

  function connect() {
    disconnect();
    setConnection("connecting");
    lostConnection = false;
    const params = new URLSearchParams();
    if (searchParams.event_type) {
      params.set("event_type", searchParams.event_type);
    }
    if (searchParams.sensor) {
      params.set("sensor", searchParams.sensor);
    }
    if (searchParams.q) {
      params.set("q", searchParams.q);
    }
    const query = params.toString();
    const url = "api/firehose/sse" + (query ? `?${query}` : "");
    console.log(`Live: connecting to ${url}`);
    const es = new EventSource(url);
    eventSource = es;
    es.onopen = () => {
      // Only mark a gap if there was something to miss; a failed first
      // connection that then succeeds is not a gap.
      if (lostConnection && received() > 0) {
        queue(gapRow(0, [REASON_RECONNECT]));
      }
      lostConnection = false;
      setConnection("open");
    };
    es.onerror = () => {
      if (es.readyState === EventSource.CLOSED) {
        // The browser gave up, typically a 4xx such as a bad filter.
        setConnection("closed");
      } else {
        // The browser will retry on its own.
        lostConnection = true;
        setConnection("reconnecting");
      }
    };
    es.onmessage = onMessage;
    es.addEventListener("gap", onGap);
  }

  function disconnect() {
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
  }

  function clear() {
    incoming = [];
    incomingEvents = 0;
    pausedDropped = 0;
    pausedReasons = new Set();
    setPendingCount(0);
    setReceived(0);
    setDropped(0);
    lastReceived = 0;
    setRows([]);
    setExpanded(new Set<number>());
  }

  function togglePaused() {
    setPaused((p) => !p);
    if (!paused()) {
      flush();
    }
  }

  function toggleExpanded(seq: number) {
    setExpanded((set) => {
      if (set.has(seq)) {
        set.delete(seq);
      } else {
        set.add(seq);
      }
      return set;
    });
  }

  function isInput(e: KeyboardEvent): boolean {
    const tag = (e.target as HTMLElement)?.tagName;
    return ["INPUT", "TEXTAREA", "SELECT"].includes(tag);
  }

  function loadEventTypes() {
    API.fetchAgg({
      field: "event_type",
      time_range: EVENT_TYPE_LOOKBACK,
      size: 100,
    })
      .then((response) => {
        const seen = new Set(
          response.rows
            .map((row) => String(row.key))
            .filter((key) => key.length > 0),
        );
        if (seen.size === 0) {
          return;
        }
        // Keep whatever is selected in the URL even if not seen recently.
        if (searchParams.event_type) {
          seen.add(searchParams.event_type);
        }
        const order = new Map(
          EVENT_TYPES.map((eventType, index) => [eventType.eventType, index]),
        );
        const types = [...seen]
          .sort((a, b) => {
            const aOrder = order.get(a) ?? EVENT_TYPES.length;
            const bOrder = order.get(b) ?? EVENT_TYPES.length;
            return aOrder - bOrder || a.localeCompare(b);
          })
          .map((eventType) => ({
            name: eventNameFromType(eventType) || eventType,
            eventType,
          }));
        setEventTypes([{ name: "All", eventType: "" }, ...types]);
      })
      .catch((error) => {
        console.log(`Failed to load event types: ${error}`);
      });
  }

  onMount(() => {
    loadEventTypes();
    flushTimer = window.setInterval(flush, FLUSH_INTERVAL_MS);
    rateTimer = window.setInterval(() => {
      const now = received();
      setRate(Math.max(0, now - lastReceived));
      lastReceived = now;
    }, 1000);
    keybindings = tinykeys(window, {
      p: (e) => {
        if (isInput(e)) return;
        togglePaused();
      },
      "/": (e) => {
        if (isInput(e)) return;
        document.getElementById("liveFilterInput")?.focus();
        e.preventDefault();
      },
    });
  });

  onCleanup(() => {
    disconnect();
    if (flushTimer !== undefined) {
      window.clearInterval(flushTimer);
    }
    if (rateTimer !== undefined) {
      window.clearInterval(rateTimer);
    }
    if (keybindings) {
      keybindings();
    }
  });

  // Reconnect when any server side filter changes. Clear the view as the
  // old events may no longer match.
  createEffect(
    on(
      () => [searchParams.event_type, searchParams.sensor, searchParams.q],
      () => {
        clear();
        connect();
      },
    ),
  );

  // Apply a reduced row limit immediately. `trim` reads `maxRows`, so this
  // re-runs when the limit changes.
  createEffect(() => {
    setRows((current) => trim(current));
  });

  const filtered = createMemo(
    () => !!(searchParams.event_type || searchParams.sensor || searchParams.q),
  );

  return (
    <>
      <Top disableRange />
      <Container fluid>
        <Row>
          <div class="col-auto mt-2 d-flex flex-wrap align-items-center gap-2">
            <button
              type="button"
              classList={{
                btn: true,
                "btn-secondary": !paused(),
                "btn-warning": paused(),
              }}
              style={"width: 6.5em;"}
              onclick={(e) => {
                togglePaused();
                e.currentTarget.blur();
              }}
              title="Pause or resume the live stream (p)"
            >
              {paused() ? "Resume" : "Pause"}
            </button>
            <button
              type="button"
              class="btn btn-secondary"
              onclick={(e) => {
                clear();
                e.currentTarget.blur();
              }}
              title="Clear all events from the view"
            >
              Clear
            </button>
            <div class="d-inline-flex">
              <SensorSelect
                selected={searchParams.sensor}
                onchange={(sensor) => {
                  setSearchParams({ sensor });
                }}
              />
            </div>
            <div class="d-inline-flex">
              <div class="input-group">
                <label class="input-group-text">Event Type</label>
                <select
                  class="form-select"
                  onchange={(e) => {
                    const eventType = e.currentTarget.value;
                    setSearchParams({ event_type: eventType || undefined });
                    e.currentTarget.blur();
                  }}
                >
                  <For each={eventTypes()}>
                    {(eventType) => (
                      <option
                        value={eventType.eventType}
                        selected={
                          eventType.eventType ===
                          (searchParams.event_type || "")
                        }
                      >
                        {eventType.name}
                      </option>
                    )}
                  </For>
                </select>
              </div>
            </div>
            <div class="d-inline-flex">
              <div class="input-group">
                <label class="input-group-text">Max Rows</label>
                <select
                  class="form-select"
                  onchange={(e) => {
                    setMaxRows(Number(e.currentTarget.value));
                    e.currentTarget.blur();
                  }}
                >
                  <For each={MAX_ROW_OPTIONS}>
                    {(n) => (
                      <option value={n} selected={n === maxRows()}>
                        {n}
                      </option>
                    )}
                  </For>
                </select>
              </div>
            </div>
          </div>
          <Col class="mt-2">
            <form
              class="input-group"
              onsubmit={(e) => {
                e.preventDefault();
                const input = e.currentTarget.elements.namedItem(
                  "q",
                ) as HTMLInputElement;
                const q = input.value.trim();
                setSearchParams({ q: q || undefined });
                input.blur();
              }}
            >
              <input
                id="liveFilterInput"
                name="q"
                type="text"
                class="form-control"
                placeholder='Filter, e.g. src_ip:10.1.1.1, @ip:10.1.1.1, @sid:2016149, "ET INFO"'
                value={searchParams.q || ""}
                onkeydown={(e) => {
                  if (e.key === "Escape") {
                    e.currentTarget.blur();
                  }
                  e.stopPropagation();
                }}
              />
              <button class="btn btn-secondary" type="submit">
                Apply
              </button>
              <button
                class="btn btn-secondary"
                type="button"
                onclick={() => setSearchParams({ q: undefined })}
              >
                Clear
              </button>
            </form>
          </Col>
        </Row>

        <Row>
          <Col class="mt-2">
            <span
              classList={{
                badge: true,
                "me-2": true,
                "text-bg-success": connection() === "open",
                "text-bg-secondary": connection() === "connecting",
                "text-bg-warning": connection() === "reconnecting",
                "text-bg-danger": connection() === "closed",
              }}
            >
              <Switch>
                <Match when={connection() === "open"}>Connected</Match>
                <Match when={connection() === "connecting"}>
                  Connecting...
                </Match>
                <Match when={connection() === "reconnecting"}>
                  Disconnected, reconnecting...
                </Match>
                <Match when={connection() === "closed"}>
                  Connection failed, check the filter or reload
                </Match>
              </Switch>
            </span>
            <Show when={paused()}>
              <span class="badge text-bg-warning me-2">Paused</span>
            </Show>
            <span
              class="badge me-2 font-monospace"
              classList={{
                "text-bg-primary": rate() > 0,
                "text-bg-secondary": rate() === 0,
              }}
              title="Events per second"
            >
              {rate()}/s
            </span>
            <span class="text-body-secondary small">
              {received().toLocaleString()} received, {eventRows().length} shown
              <Show when={paused() && pendingCount() > 0}>
                , {pendingCount().toLocaleString()} waiting
              </Show>
              <Show when={dropped() > 0}>
                ,{" "}
                <span class="text-danger">
                  {dropped().toLocaleString()} dropped
                </span>
              </Show>
            </span>
          </Col>
        </Row>

        <Row>
          <Col class="mt-2">
            <Show
              when={rows().length > 0}
              fallback={
                <div class="text-body-secondary mt-3">
                  <Show when={filtered()} fallback={<>Waiting for events...</>}>
                    Waiting for matching events...
                  </Show>
                </div>
              }
            >
              <table class="table table-sm table-hover app-event-table">
                <thead>
                  <tr>
                    <th class="app-col-timestamp">Timestamp</th>
                    <th class="app-col-event-type">Type</th>
                    <th class="app-col-event-type">Sensor</th>
                    <th class="app-col-address">Src/Dst</th>
                    <th>Description</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={rows()}>
                    {(row) => (
                      <Switch>
                        <Match when={row.kind === "gap"}>
                          <tr class="app-live-gap">
                            <td colspan={5} class="small text-body-secondary">
                              <span class="me-2">&#9888;</span>
                              {gapMessage(row as LiveGapRow)}
                              <span class="ms-2 text-body-tertiary">
                                {(row as LiveGapRow).at}
                              </span>
                            </td>
                          </tr>
                        </Match>
                        <Match when={row.kind === "event"}>
                          <LiveEventRows
                            row={row as LiveEventRow}
                            expanded={expanded().has(row.seq)}
                            onclick={() => toggleExpanded(row.seq)}
                          />
                        </Match>
                      </Switch>
                    )}
                  </For>
                </tbody>
              </table>
            </Show>
          </Col>
        </Row>
      </Container>
    </>
  );
}

function LiveEventRows(props: {
  row: LiveEventRow;
  expanded: boolean;
  onclick: () => void;
}) {
  const event = props.row.event;
  const source = event._source;
  const severity = source.alert?.severity;
  // Only alerts get a colored background; other events stay neutral so
  // alerts stand out, and no safety judgement is implied.
  const isAlert = source.event_type === "alert";
  const addresses = source.src_ip ? source : source.arp;

  return (
    <>
      <tr
        class="app-live-row"
        style={"cursor: pointer;"}
        onclick={props.onclick}
        classList={{
          "table-info": isAlert && severity! > 2,
          "table-warning": isAlert && severity === 2,
          "table-danger": isAlert && severity === 1,
        }}
      >
        <td class="app-col-timestamp">
          <TimestampCell timestamp={source.timestamp} />
        </td>
        <td class="app-col-event-type">
          {source.event_type?.toUpperCase() || "???"}
        </td>
        <td class="app-col-event-type">{source.host}</td>
        <td class="app-col-address" style={"width: 0%;"}>
          <Show when={addresses?.src_ip}>
            S: {formatAddressWithPort(addresses.src_ip, addresses.src_port)}
            <br />
          </Show>
          <Show when={addresses?.dest_ip}>
            D: {formatAddressWithPort(addresses.dest_ip, addresses.dest_port)}
          </Show>
        </td>
        <td class="app-break-anywhere col-auto">
          <Switch
            fallback={
              <>
                {formatEventDescription(event)}
                <AppProtoBadge event={event} class={"ms-2"} />
              </>
            }
          >
            <Match when={isAlert}>
              <AlertDescription event={event} />
            </Match>
          </Switch>
        </td>
      </tr>
      <Show when={props.expanded}>
        <tr>
          <td colspan={5}>
            <pre class="mb-0 small" style={"white-space: pre-wrap;"}>
              {JSON.stringify(source, null, 2)}
            </pre>
          </td>
        </tr>
      </Show>
    </>
  );
}
