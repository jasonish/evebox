// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Button } from "solid-bootstrap";
import { Show } from "solid-js";

export function RefreshButton(props: {
  loading: number;
  refresh: () => void;
  showProgress?: boolean;
}) {
  return (
    <>
      <Button
        style={"width: 7em;"}
        disabled={props.loading > 0}
        onclick={props.refresh}
        classList={{ "ps-0": props.loading > 0 }}
      >
        <Show when={props.loading == 0}>Refresh</Show>
        <Show when={props.loading > 0}>
          Loading
          <Show when={props.showProgress}>:{props.loading}</Show>
        </Show>
      </Button>
    </>
  );
}
