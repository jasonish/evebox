// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// The events Suricata logged for the same flow as the viewed event, as a
// timeline with a summary from the flow record. Alerts are grouped by
// signature and runs of identical events are collapsed, so a flow that
// raises the same alert thousands of times stays one row.

import { useLocation, useNavigate } from "@solidjs/router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
  JSX,
  on,
  Show,
} from "solid-js";
import { FlowEventsResponse, getFlowEvents } from "./api";
import { RawSearchLink } from "./common/SearchLink";
import { FormattedTimestamp } from "./components";
import { parse_timestamp } from "./datetime";
import { AppProtoBadge, eventNameFromType } from "./Events";
import { formatBytes, formatEventDescription } from "./formatters";
import { BiCaretRightFill } from "./icons";
import { EventWrapper } from "./types";

// Rows shown before the listing is expanded.
const COLLAPSED_ROWS = 10;

// One timeline row: a single event, or several alike ones.
interface FlowRow {
  // The first event of the row.
  event: EventWrapper;
  count: number;
  lastTimestamp: string;
  // IDs of the events in the row, when known (not for alert groups).
  ids: string[];
  // Search for all the events of the row.
  query: string;
}

export function FlowEvents(props: { event: EventWrapper }) {
  const navigate = useNavigate();
  const location = useLocation();

  // Only events with a flow ID have flow events to look up.
  const eventId = () =>
    props.event._source.flow_id != null ? String(props.event._id) : undefined;

  const [flow] = createResource(eventId, async (id) => {
    try {
      return { id, ...(await getFlowEvents(id)) };
    } catch (err) {
      console.log(`Failed to fetch flow events for ${id}: ${err}`);
      return undefined;
    }
  });

  const flowQuery = () => `flow_id:${props.event._source.flow_id}`;

  // While a new event's flow loads, keep showing the previous one only if
  // it is the same flow, to avoid flicker when stepping through it.
  const response = createMemo(() => {
    const r = flow.latest;
    if (
      !r ||
      (r.id !== eventId() && r.flow_id !== String(props.event._source.flow_id))
    ) {
      return undefined;
    }
    return r;
  });

  const rows = createMemo(() => {
    const r = response();
    return r ? buildRows(r, flowQuery(), props.event) : [];
  });

  // Long listings show their first rows, plus the current event's,
  // until expanded.
  const [expanded, setExpanded] = createSignal(false);
  createEffect(
    on(
      () => props.event._source.flow_id,
      () => setExpanded(false),
    ),
  );
  const collapsible = () => rows().length > COLLAPSED_ROWS + 2;
  const visibleRows = () =>
    expanded() || !collapsible()
      ? rows()
      : rows().filter((row, i) => i < COLLAPSED_ROWS || isCurrent(row));

  const total = () =>
    Object.values(response()?.counts ?? {}).reduce((a, b) => a + b, 0);

  const isCurrent = (row: FlowRow) => {
    const current = props.event;
    if (row.event._source.event_type === "alert") {
      return (
        current._source.event_type === "alert" &&
        row.event._source.alert?.signature_id ===
          current._source.alert?.signature_id
      );
    }
    return row.ids.includes(String(current._id));
  };

  // Offsets are relative to the start of the flow when known.
  const origin = () =>
    response()?.flow?._source.flow?.start ??
    props.event._source.flow?.start ??
    rows()[0]?.event._source.timestamp;

  const nonAlertCount = () => total() - (response()?.counts["alert"] ?? 0);

  function openEvent(event: EventWrapper) {
    if (String(event._id) === String(props.event._id)) {
      return;
    }
    navigate(`/event/${event._id}`, {
      state: {
        referer: location.pathname,
      },
    });
  }

  return (
    <Show when={total() > 1}>
      <div class="card mb-2">
        <div class="card-header d-flex justify-content-between align-items-center">
          <span>
            Flow
            <span class="text-body-secondary ms-2 small">
              {formatCounts(response()?.counts ?? {})}
            </span>
          </span>
          <span class="small">
            <RawSearchLink q={flowQuery()}>Search</RawSearchLink>
          </span>
        </div>
        <Show when={response()?.flow}>
          <FlowSummary event={response()!.flow!} />
        </Show>
        <div class="card-body p-0">
          <table class="table table-sm table-hover app-event-table mb-0">
            <thead>
              <tr>
                <th class="app-w-1"></th>
                <th class="app-col-timestamp">Timestamp</th>
                <th class="app-col-timestamp" style="text-align: right;">
                  Offset
                </th>
                <th class="app-col-event-type">Type</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              <For each={visibleRows()}>
                {(row) => {
                  const event = row.event;
                  const severity = event._source.alert?.severity;
                  return (
                    <tr
                      onclick={() => openEvent(event)}
                      classList={{
                        "table-info": severity! > 2,
                        "table-warning": severity === 2,
                        "table-danger": severity === 1,
                        "table-success": severity === undefined,
                      }}
                    >
                      <td class="app-w-1" style="min-width: 1.5em !important;">
                        <Show when={isCurrent(row)}>
                          <BiCaretRightFill />
                        </Show>
                      </td>
                      <td class="app-col-timestamp">
                        <FormattedTimestamp
                          timestamp={event._source.timestamp}
                          withMillis={true}
                        />
                      </td>
                      <td
                        class="app-col-timestamp"
                        style="text-align: right; font-variant-numeric: tabular-nums;"
                      >
                        {formatOffset(origin(), event._source.timestamp)}
                      </td>
                      <td class="app-col-event-type">
                        {eventTypeName(event._source.event_type)}
                      </td>
                      <td class="app-break-anywhere">
                        <Show when={row.count > 1}>
                          <span
                            class="badge text-bg-secondary me-2"
                            title={`${row.count.toLocaleString()} events, last at ${row.lastTimestamp}`}
                            onclick={(e) => e.stopPropagation()}
                          >
                            <RawSearchLink q={row.query}>
                              <span class="text-white">
                                ×{row.count.toLocaleString()}
                              </span>
                            </RawSearchLink>
                          </span>
                        </Show>
                        {formatEventDescription(event)}
                        <Show when={event._source.event_type !== "alert"}>
                          <AppProtoBadge event={event} class="ms-2" />
                        </Show>
                        <Show when={row.count > 1}>
                          <span class="text-body-secondary ms-2">
                            over{" "}
                            {formatMillis(
                              elapsed(
                                event._source.timestamp,
                                row.lastTimestamp,
                              ),
                            )}
                          </span>
                        </Show>
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
          <Show when={collapsible()}>
            <div class="small px-2 py-1 border-bottom">
              <button
                type="button"
                class="btn btn-link btn-sm p-0"
                onclick={() => setExpanded(!expanded())}
              >
                {expanded()
                  ? "Show fewer"
                  : `Show all ${rows().length.toLocaleString()} rows`}
              </button>
            </div>
          </Show>
          <Show when={response()?.truncated}>
            <div class="small text-body-secondary px-2 py-1">
              Showing the first {response()!.events.length} of{" "}
              {nonAlertCount().toLocaleString()} non-alert events.{" "}
              <RawSearchLink q={`${flowQuery()} -event_type:alert`}>
                Search all
              </RawSearchLink>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  );
}

// Merge the alert groups and other events into timeline rows, collapsing
// consecutive events of the same type and description. The current event
// is added if a truncated listing left it out.
function buildRows(
  response: FlowEventsResponse,
  flowQuery: string,
  current: EventWrapper,
) {
  const rows: FlowRow[] = response.alerts.map((group) => ({
    event: group.first,
    count: group.count,
    lastTimestamp: group.last_timestamp,
    ids: [],
    query: `${flowQuery} alert.signature_id:${group.first._source.alert?.signature_id}`,
  }));

  let events = response.events;
  if (
    current._source.event_type !== "alert" &&
    !events.some((event) => String(event._id) === String(current._id))
  ) {
    events = [...events, current].sort((a, b) =>
      elapsed(b._source.timestamp, a._source.timestamp),
    );
  }

  let previous: { row: FlowRow; key: string } | undefined;
  for (const event of events) {
    const key = `${event._source.event_type}|${formatEventDescription(event)}`;
    if (previous && previous.key === key) {
      previous.row.count += 1;
      previous.row.lastTimestamp = event._source.timestamp;
      previous.row.ids.push(String(event._id));
      continue;
    }
    const row = {
      event,
      count: 1,
      lastTimestamp: event._source.timestamp,
      ids: [String(event._id)],
      query: `${flowQuery} event_type:${event._source.event_type}`,
    };
    rows.push(row);
    previous = { row, key };
  }

  return rows.sort((a, b) =>
    elapsed(b.event._source.timestamp, a.event._source.timestamp),
  );
}

// A one-line summary of a flow record: duration, traffic in each
// direction and how the flow ended.
function FlowSummary(props: { event: EventWrapper }) {
  const items = createMemo(() => {
    const f = props.event._source.flow!;
    const items: [string, JSX.Element][] = [];
    if (f.start && f.end) {
      items.push(["Duration", formatMillis(elapsed(f.start, f.end))]);
    }
    items.push([
      "To server",
      `${(f.pkts_toserver ?? 0).toLocaleString()} pkts, ${formatBytes(
        f.bytes_toserver ?? 0,
      )}`,
    ]);
    items.push([
      "To client",
      `${(f.pkts_toclient ?? 0).toLocaleString()} pkts, ${formatBytes(
        f.bytes_toclient ?? 0,
      )}`,
    ]);
    if (props.event._source.app_proto) {
      items.push(["App", props.event._source.app_proto]);
    }
    if (f.state) {
      items.push(["State", f.state]);
    }
    if (f.reason) {
      items.push(["Reason", f.reason]);
    }
    return items;
  });

  return (
    <div class="card-body py-1 small border-bottom">
      <For each={items()}>
        {([label, value]) => (
          <span class="me-3 text-nowrap">
            <span class="text-body-secondary">{label}:</span> {value}
          </span>
        )}
      </For>
    </div>
  );
}

// For example "5,180 alerts, 1 TLS, 1 Flow".
function formatCounts(counts: { [eventType: string]: number }): string {
  return Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([eventType, count]) => {
      const name =
        eventType === "alert"
          ? count === 1
            ? "alert"
            : "alerts"
          : eventTypeName(eventType);
      return `${count.toLocaleString()} ${name}`;
    })
    .join(", ");
}

function eventTypeName(eventType: string): string {
  if (eventType === "fileinfo") {
    return "File Info";
  }
  return eventNameFromType(eventType) || eventType?.toUpperCase() || "???";
}

function elapsed(from: string, to: string): number {
  return parse_timestamp(to).diff(parse_timestamp(from));
}

// Elapsed time from the start of the flow, e.g. "+1.234s".
function formatOffset(origin: string | undefined, timestamp: string): string {
  if (!origin) {
    return "";
  }
  const ms = elapsed(origin, timestamp);
  if (ms <= 0) {
    return "";
  }
  return `+${formatMillis(ms)}`;
}

function formatMillis(ms: number): string {
  ms = Math.max(ms, 0);
  if (ms < 1000) {
    return `${ms}ms`;
  }
  const seconds = ms / 1000;
  if (seconds < 60) {
    return `${seconds.toFixed(3)}s`;
  }
  const total = Math.floor(seconds);
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (d > 0) {
    return `${d}d ${h}h ${m}m`;
  }
  if (h > 0) {
    return `${h}h ${m}m ${s}s`;
  }
  return `${m}m ${s}s`;
}
