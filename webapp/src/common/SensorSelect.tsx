// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createEffect, createSignal, For, Show } from "solid-js";
import { API } from "../api";
import { Button, Spinner } from "solid-bootstrap";

export function SensorSelect(props: {
  selected: string | undefined;
  onchange: (value: string | undefined) => void;
}) {
  const [sensors, setSensors] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(true);

  createEffect(() => {
    API.getSensors()
      .then((response) => {
        setSensors(response.data);
      })
      .finally(() => {
        setLoading(false);
      });
  });
  function setSensor(event: any) {
    let sensor = event.currentTarget.value;
    if (sensor === "") {
      props.onchange(undefined);
    } else {
      props.onchange(sensor);
    }
  }

  return (
    <div class="input-group">
      <label class="input-group-text">Sensor</label>
      <Show
        when={!loading()}
        fallback={
          <Button variant={"outline-secondary"} disabled>
            {"Loading "}
            <Spinner
              as="span"
              animation="grow"
              size="sm"
              role="status"
              aria-hidden="true"
            />
          </Button>
        }
      >
        <select class="form-select" onchange={setSensor}>
          <option value={""}>All</option>
          <For each={sensors()}>
            {(sensor) => (
              <option value={sensor} selected={sensor == props.selected}>
                {sensor}
              </option>
            )}
          </For>
        </select>
      </Show>
    </div>
  );
}
