// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Shared by the pages fed by the live feed (Live and Top).

import { FeedStatus } from "./api";

const SOURCES: FeedStatus["source"][] = ["ingest", "datastore"];
const STATES: FeedStatus["state"][] = ["starting", "ok", "retrying"];

// Seconds between datastore polls if the status doesn't say, the
// server's default.
const DEFAULT_POLL_INTERVAL = 1;

// While a page reading events from the datastore is hidden, such as a
// forgotten background tab, the server keeps polling for it. After this
// long hidden the page closes its connection, which stops the polling if
// nobody else is watching, and opens it again when shown.
const HIDDEN_DISCONNECT_MS = 2 * 60 * 1000;

// Call `park` once the page has been hidden for HIDDEN_DISCONNECT_MS while
// `datastore()` holds, and `resume` when it is shown again after that.
// Events that EveBox ingests cost nothing to keep receiving, so pages fed
// by ingest stay connected while hidden, checking again every
// HIDDEN_DISCONNECT_MS, as the source can change to the datastore while
// the page stays hidden, such as when ingest stops.
//
// Also parks, whatever the source, when the page is navigated away from:
// the browser can keep it, connection and all, in its back/forward cache,
// where its timers don't run, and resumes it if it is brought back.
//
// Returns a function that stops watching.
export function parkWhileHidden(
  datastore: () => boolean,
  park: () => void,
  resume: () => void,
): () => void {
  let timer: number | undefined;
  let parked = false;
  const cancel = () => {
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timer = undefined;
    }
  };
  const doPark = () => {
    cancel();
    if (!parked) {
      parked = true;
      park();
    }
  };
  const doResume = () => {
    cancel();
    if (parked) {
      parked = false;
      resume();
    }
  };
  const check = () => {
    timer = undefined;
    if (parked || document.visibilityState !== "hidden") {
      return;
    }
    if (datastore()) {
      doPark();
    } else {
      timer = window.setTimeout(check, HIDDEN_DISCONNECT_MS);
    }
  };
  const onVisibilityChange = () => {
    if (document.visibilityState === "hidden") {
      cancel();
      timer = window.setTimeout(check, HIDDEN_DISCONNECT_MS);
    } else {
      doResume();
    }
  };
  const onPageShow = (event: PageTransitionEvent) => {
    if (event.persisted) {
      doResume();
    }
  };
  document.addEventListener("visibilitychange", onVisibilityChange);
  window.addEventListener("pagehide", doPark);
  window.addEventListener("pageshow", onPageShow);
  if (document.visibilityState === "hidden") {
    onVisibilityChange();
  }
  return () => {
    cancel();
    document.removeEventListener("visibilitychange", onVisibilityChange);
    window.removeEventListener("pagehide", doPark);
    window.removeEventListener("pageshow", onPageShow);
  };
}

// Decode a "status" message from the firehose SSE stream. Returns null for
// anything malformed, which the caller ignores, keeping the last status.
export function parseFeedStatus(message: MessageEvent): FeedStatus | null {
  let data: any;
  try {
    data = JSON.parse(message.data);
  } catch (e) {
    console.log(`Failed to decode live feed status: ${e}`);
    return null;
  }
  if (
    !data ||
    typeof data !== "object" ||
    !SOURCES.includes(data.source) ||
    !STATES.includes(data.state)
  ) {
    console.log(`Unexpected live feed status: ${message.data}`);
    return null;
  }
  const number = (v: any): number | undefined =>
    typeof v === "number" && Number.isFinite(v) ? v : undefined;
  return {
    source: data.source,
    state: data.state,
    interval: number(data.interval),
    lookback: number(data.lookback),
    behind: data.behind === true,
    partial: typeof data.partial === "string" ? data.partial : undefined,
    error: typeof data.error === "string" ? data.error : undefined,
  };
}

// Events per second, from a count of events received sampled about once
// a second. Measured against the clock rather than assuming a one second
// tick, as timers run late in hidden pages.
//
// Events read from the datastore arrive in one burst per poll, so over
// one second the rate would swing between 0 and about twice the real
// rate. Then it is taken over two poll intervals instead, from the sample
// closest to that long ago, as with timers running a little late the
// spans either side of it hold one burst more or less.
export class EventRate {
  private samples: { at: number; count: number }[] = [];

  constructor() {
    this.reset();
  }

  // Start over, as the count went back to 0.
  reset() {
    this.samples = [{ at: performance.now(), count: 0 }];
  }

  // Sample the count, returning the rate.
  sample(count: number, feed: FeedStatus | null): number {
    const at = performance.now();
    let span = 0;
    if (feed?.source === "datastore") {
      const interval =
        feed.interval && feed.interval > 0
          ? feed.interval
          : DEFAULT_POLL_INTERVAL;
      span = 2 * interval * 1000;
    }
    // With no span, from the previous sample.
    const off = (i: number) => Math.abs(at - this.samples[i].at - span);
    let i = this.samples.length - 1;
    while (i > 0 && off(i - 1) <= off(i)) {
      i -= 1;
    }
    const from = this.samples[i];
    this.samples = this.samples.slice(i);
    this.samples.push({ at, count });
    // Not much under a second, so a tick that runs early, after one that
    // ran late, still gives the rate, while two ticks run back to back
    // can't make it spike.
    const elapsed = Math.max(0.5, (at - from.at) / 1000);
    return Math.max(0, Math.round((count - from.count) / elapsed));
  }
}
