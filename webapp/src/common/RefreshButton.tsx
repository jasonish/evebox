// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Show } from "solid-js";

export function RefreshButton(props: { loading: number; refresh: () => void }) {
  return (
    <>
      <button
        class="btn btn-primary position-relative"
        disabled={props.loading > 0}
        onclick={props.refresh}
      >
        Refresh
        <Show when={props.loading > 0}>
          <span class="position-absolute top-50 start-100 translate-middle badge rounded-pill bg-info">
            {props.loading}
            <span class="visually-hidden">unread messages</span>
          </span>
        </Show>
      </button>
    </>
  );
}
