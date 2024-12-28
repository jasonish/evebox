// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { parse_timestamp } from "./datetime";
import { EventSource } from "./types";
import { formatAddress } from "./formatters";
import { BiDashCircle, BiPlusCircle } from "./icons";
import { Show } from "solid-js";

export function TimestampCell(props: { timestamp: string }) {
  let timestamp = parse_timestamp(props.timestamp);
  return (
    <div title={props.timestamp}>
      {timestamp.format("YYYY-MM-DD HH:mm:ss")}
      <br />
      <span class={"small"}>{timestamp.fromNow()}</span>
    </div>
  );
}

export function AddressCell(props: {
  source: EventSource;
  fn?: (what: string, op: string, value: string | number) => void;
}) {
  try {
    return (
      <>
        <Show when={props.source.src_ip && props.source.src_ip.length > 0}>
          S: {formatAddress(props.source.src_ip)}
          <Show when={props.fn}>
            <span
              class="show-on-hover ms-1"
              onClick={(e) => {
                e.stopPropagation();
                props.fn!("src_ip", "+", props.source.src_ip);
              }}
              title="Filter for this src_ip"
            >
              <BiPlusCircle />
            </span>
            <span
              class="show-on-hover ms-1"
              onClick={(e) => {
                e.stopPropagation();
                props.fn!("src_ip", "-", props.source.src_ip);
              }}
              title="Filter out this src_ip"
            >
              <BiDashCircle />
            </span>
          </Show>
          <br />
        </Show>
        <Show when={props.source.dest_ip && props.source.dest_ip.length > 0}>
          D: {formatAddress(props.source.dest_ip)}
          <Show when={props.fn}>
            <span
              class="show-on-hover ms-1"
              onClick={(e) => {
                e.stopPropagation();
                props.fn!("dest_ip", "+", props.source.dest_ip);
              }}
              title="Filter for this dest_ip"
            >
              <BiPlusCircle />
            </span>
            <span
              class="show-on-hover ms-1"
              onClick={(e) => {
                e.stopPropagation();
                props.fn!("dest_ip", "-", props.source.dest_ip);
              }}
              title="Filter out this dest_ip"
            >
              <BiDashCircle />
            </span>
          </Show>
        </Show>
      </>
    );
  } catch (e) {
    console.log(e);
    return <>`Failed to format address: ${e}`</>;
  }
}
