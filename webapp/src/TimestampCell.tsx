// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { parse_timestamp } from "./datetime";
import { EventSource } from "./types";
import { formatAddress } from "./formatters";

export function TimestampCell(props: { timestamp: string }) {
  let timestamp = parse_timestamp(props.timestamp);
  return (
    <>
      {timestamp.format("YYYY-MM-DD HH:mm:ss")}
      <br />
      <span class={"small"}>{timestamp.fromNow()}</span>
    </>
  );
}

export function AddressCell(props: { source: EventSource }) {
  return (
    <>
      S: {formatAddress(props.source.src_ip)}
      <br />
      D: {formatAddress(props.source.dest_ip)}
    </>
  );
}
