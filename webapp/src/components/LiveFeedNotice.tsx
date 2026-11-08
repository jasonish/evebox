// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createMemo, Match, Show, Switch } from "solid-js";
import { FeedStatus } from "../api";
import { distributionName, serverConfig } from "../config";
import { BiInfoCircle } from "../icons";

// A notice for the pages fed by the firehose (Live and Top) about where
// their events come from. It follows the feed's status, which the server
// sends first on every connection. Nothing is shown until it has arrived,
// so the wrong notice never flashes.
//
// With Elasticsearch or OpenSearch and nothing ingested through EveBox,
// typically because another tool such as Filebeat or Logstash writes the
// events, the server reads new events from the datastore every few
// seconds while someone is watching. That is best effort: events stored
// too late, or more than a poll can read, are missed. So it is always
// noted, quietly, with a warning while reading fails, falls behind, or
// part of the datastore isn't answering.
//
// Otherwise the events are those EveBox ingests, and a notice is shown
// while the server has ingested nothing. The server reports how many
// events have passed through it since it started (refreshed with the
// config every few seconds). The wording states what the server knows
// rather than concluding nothing will ever arrive: a legacy agent that
// posts events without a persistent connection isn't counted as
// connected, so "no agents" doesn't mean "no agents will submit".
export function LiveFeedNotice(props: {
  // Events the page has received, so the notice clears as soon as one
  // arrives without waiting for the next config refresh.
  received: number;
  // The feed's last status, null until the first one arrives.
  feed: FeedStatus | null;
  // The page counts events rather than listing them, so the note also
  // says when they are counted.
  counts?: boolean;
}) {
  const datastore = () =>
    props.feed?.source === "datastore" ? props.feed : null;

  return (
    <Switch>
      <Match when={datastore()}>
        {(feed) => <DatastoreFeedNote feed={feed()} counts={props.counts} />}
      </Match>
      <Match when={props.feed?.source === "ingest"}>
        <NoIngestNotice received={props.received} />
      </Match>
    </Switch>
  );
}

// The poll interval for display, as in "every second".
function every(secs: number | undefined): string {
  if (secs === undefined || secs <= 0) {
    return "every few seconds";
  }
  if (secs === 1) {
    return "every second";
  }
  const n = secs.toLocaleString(undefined, { maximumFractionDigits: 1 });
  return `every ${n} seconds`;
}

function DatastoreFeedNote(props: { feed: FeedStatus; counts?: boolean }) {
  const retrying = () => props.feed.state === "retrying";

  return (
    <div class="mt-2 small">
      <div class="text-body-secondary">
        <BiInfoCircle class="me-1 align-text-bottom" />
        Events are read from {distributionName()} {every(props.feed.interval)},
        as none are ingested through EveBox. This is best effort: events can be
        missed or show up late
        {props.counts ? ", and they are counted when EveBox reads them." : "."}
      </div>
      <Show when={retrying() || props.feed.behind}>
        <div class="alert alert-warning py-1 px-2 mt-1 mb-0" role="status">
          <Show when={retrying()}>
            <div>
              Unable to read events from {distributionName()}, retrying
              {props.feed.error ? `: ${props.feed.error}` : "."}
            </div>
          </Show>
          <Show
            when={props.feed.partial}
            fallback={
              <Show when={props.feed.behind}>
                <div>
                  More events are arriving than can be read, so some are being
                  skipped.
                </div>
              </Show>
            }
          >
            <div>
              Part of {distributionName()} isn't answering, so events stored
              there are being skipped: {props.feed.partial}
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
}

function NoIngestNotice(props: { received: number }) {
  const noSource = createMemo(() => {
    const live = serverConfig()?.live;
    return live !== undefined && live.ingested === 0 && props.received === 0;
  });
  const noSourceConfigured = createMemo(() => {
    const live = serverConfig()?.live;
    return live !== undefined && !live.inputs && live.agents === 0;
  });

  return (
    <Show when={noSource()}>
      <div class="alert alert-warning mt-2 mb-0" role="alert">
        <Show
          when={noSourceConfigured()}
          fallback={
            <>
              <strong>No events have reached this server yet.</strong> This page
              only shows events that pass through EveBox, from its EVE file or
              socket inputs or from agents.
            </>
          }
        >
          <strong>This server has no event sources.</strong> No EVE file or
          socket inputs are configured, no agents are connected, and no events
          have been submitted since it started.
        </Show>
      </div>
    </Show>
  );
}
