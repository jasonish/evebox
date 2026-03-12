// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Show } from "solid-js";

export function RefreshButton(props: { loading: number; refresh: () => void }) {
  return (
    <span class="position-relative d-inline-flex app-refresh-button-wrap">
      <button
        class="btn btn-primary position-relative app-refresh-button"
        disabled={props.loading > 0}
        onclick={props.refresh}
      >
        Refresh
      </button>
      <Show when={props.loading > 0}>
        <span class="position-absolute top-50 start-100 translate-middle badge rounded-pill bg-info app-refresh-button-badge">
          {props.loading}
          <span class="visually-hidden">unread messages</span>
        </span>
      </Show>
    </span>
  );
}
