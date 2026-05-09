// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Show } from "solid-js";

export function RefreshButton(props: {
  loading: boolean | number;
  refresh: () => void;
}) {
  const isLoading = () => {
    return typeof props.loading === "number"
      ? props.loading > 0
      : props.loading;
  };

  const loadingCount = () => {
    return typeof props.loading === "number" ? props.loading : undefined;
  };

  return (
    <span class="position-relative d-inline-flex app-refresh-button-wrap">
      <button
        class="btn btn-primary position-relative app-refresh-button"
        disabled={isLoading()}
        onclick={props.refresh}
      >
        {isLoading() && loadingCount() === undefined ? "Loading" : "Refresh"}
      </button>
      <Show when={loadingCount() !== undefined && loadingCount()! > 0}>
        <span class="position-absolute top-50 start-100 translate-middle badge rounded-pill bg-info app-refresh-button-badge">
          {loadingCount()}
          <span class="visually-hidden">unread messages</span>
        </span>
      </Show>
    </span>
  );
}
