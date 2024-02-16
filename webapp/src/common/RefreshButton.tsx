// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Button } from "solid-bootstrap";
import { Show } from "solid-js";
import { Transition } from "solid-transition-group";

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
      >
        <div style={"position: relative; height: 24px;"}>
          <Transition name={"fade"}>
            {(props.loading > 0 && (
              <span
                style={
                  "position: absolute; top: 0; bottom: 0; left: 3.5em; margin-left: -3em;"
                }
              >
                Loading
                <Show when={props.showProgress}>:{props.loading}</Show>
              </span>
            )) || (
              <>
                <span
                  style={
                    "position: absolute; top: 0; bottom: 0; left: 3.5em; margin-left: -2.7em;"
                  }
                >
                  Refresh
                </span>
              </>
            )}
          </Transition>
        </div>
      </Button>
    </>
  );
}
