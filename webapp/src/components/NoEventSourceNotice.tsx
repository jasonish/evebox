// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createMemo, Show } from "solid-js";
import { serverConfig } from "../config";

// A notice for the pages fed by the firehose (Live and Top), shown while
// the server has ingested nothing. The server reports how many events have
// passed through it since it started (refreshed with the config every few
// seconds). While that is zero nothing can show, typically because another
// tool writes events straight to the datastore. Only events that pass
// through EveBox reach the firehose. The wording states what the server
// knows rather than concluding nothing will ever arrive: a legacy agent
// that posts events without a persistent connection isn't counted as
// connected, so "no agents" doesn't mean "no agents will submit".
export function NoEventSourceNotice(props: {
  // Events the page has received, so the notice clears as soon as one
  // arrives without waiting for the next config refresh.
  received: number;
}) {
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
          have been submitted since it started. This page only shows events that
          pass through EveBox, so events that other tools write directly to
          Elasticsearch or OpenSearch will not appear here.
        </Show>
      </div>
    </Show>
  );
}
