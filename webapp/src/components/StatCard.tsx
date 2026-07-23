// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Show } from "solid-js";

// A compact statistic card: a large value, an uppercase label, and an
// optional sub-line such as a percentage.
export function StatCard(props: {
  value: string | null;
  label: string;
  sub?: string;
}) {
  return (
    <div class="card h-100">
      <div class="card-body d-flex flex-column align-items-center justify-content-center px-3 py-2 text-center">
        <span class="fs-4 fw-semibold lh-sm">{props.value ?? "-"}</span>
        <span class="app-event-summary-label mt-1 text-body-secondary fw-medium lh-sm text-uppercase">
          {props.label}
        </span>
        <Show when={props.sub}>
          <span class="text-body-secondary small lh-sm">{props.sub}</span>
        </Show>
      </div>
    </div>
  );
}
