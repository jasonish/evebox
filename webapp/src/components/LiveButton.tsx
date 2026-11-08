// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

export type LiveConnection = "connecting" | "open" | "reconnecting" | "closed";

// The pause button for the pages fed by the firehose (Live and Top). It
// doubles as the connection status: "Live" while connected and running,
// otherwise paused, connecting or closed.
export function LiveButton(props: {
  paused: boolean;
  connection: LiveConnection;
  onclick: () => void;
  // Keyboard shortcut to mention in the tooltip, if the page has one.
  shortcut?: string;
}) {
  const status = (): "live" | "paused" | "connecting" | "closed" =>
    props.paused
      ? "paused"
      : props.connection === "open"
        ? "live"
        : props.connection === "closed"
          ? "closed"
          : "connecting";

  const title = () => {
    const text = props.paused
      ? "Paused, click to resume"
      : status() === "closed"
        ? "Disconnected from the server"
        : "Click to pause updates";
    return props.shortcut ? `${text} (${props.shortcut})` : text;
  };

  return (
    <button
      type="button"
      classList={{
        btn: true,
        "btn-success": status() === "live",
        "btn-warning": status() === "paused",
        "btn-secondary": status() === "connecting",
        "btn-danger": status() === "closed",
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
        }[status()]
      }
    </button>
  );
}
