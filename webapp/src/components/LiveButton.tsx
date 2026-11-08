// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { FeedStatus } from "../api";
import { distributionName } from "../config";

export type LiveConnection = "connecting" | "open" | "reconnecting" | "closed";

type LiveStatus =
  | "live"
  | "paused"
  | "connecting"
  | "closed"
  | "starting"
  | "retrying";

// The pause button for the pages fed by the firehose (Live and Top). It
// doubles as the connection status: "Live" while connected and running,
// otherwise paused, connecting or closed. When the events are read from
// the datastore it can also be "Starting", while the server reads what is
// already there, or "Retrying", while reading fails.
export function LiveButton(props: {
  paused: boolean;
  connection: LiveConnection;
  // The feed's last status, if the page tracks it.
  feed?: FeedStatus | null;
  onclick: () => void;
  // Keyboard shortcut to mention in the tooltip, if the page has one.
  shortcut?: string;
}) {
  const status = (): LiveStatus => {
    if (props.paused) {
      return "paused";
    }
    if (props.connection === "closed") {
      return "closed";
    }
    if (props.connection !== "open") {
      return "connecting";
    }
    if (props.feed?.source === "datastore" && props.feed.state === "starting") {
      return "starting";
    }
    if (props.feed?.state === "retrying") {
      return "retrying";
    }
    return "live";
  };

  const title = () => {
    let text: string;
    switch (status()) {
      case "paused":
        text = "Paused, click to resume";
        break;
      case "closed":
        text = "Disconnected from the server";
        break;
      case "starting":
        text =
          `Reading what is already in ${distributionName()} so that only ` +
          "new events are shown. Click to pause updates";
        break;
      case "retrying": {
        const error = props.feed?.error;
        text =
          `Unable to read events from ${distributionName()}, retrying` +
          (error ? `: ${error}` : "") +
          "\nClick to pause updates";
        break;
      }
      default:
        text = "Click to pause updates";
    }
    return props.shortcut ? `${text} (${props.shortcut})` : text;
  };

  return (
    <button
      type="button"
      classList={{
        btn: true,
        "btn-success": status() === "live",
        "btn-warning": status() === "paused",
        "btn-secondary": status() === "connecting" || status() === "starting",
        "btn-danger": status() === "closed" || status() === "retrying",
      }}
      style={"min-width: 7.5em;"}
      onclick={(e) => {
        props.onclick();
        e.currentTarget.blur();
      }}
      title={title()}
    >
      {
        {
          live: "● Live",
          paused: "Paused",
          connecting: "Connecting",
          closed: "Closed",
          starting: "Starting",
          retrying: "Retrying",
        }[status()]
      }
    </button>
  );
}
