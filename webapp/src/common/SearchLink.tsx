// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { A } from "@solidjs/router";

export function SearchLink(props: {
  children?: any;
  field?: string;
  value: any;
  class?: string;
}) {
  if (props.value === undefined) {
    return <></>;
  }

  let q;
  switch (typeof props.value) {
    case "number":
    case "boolean":
      q = encodeURIComponent(
        `${props.field ? props.field + ":" : ""}${props.value}`
      );
      break;
    default:
      let value;
      try {
        value = props.value.replaceAll('"', '\\"');
      } catch (_err) {
        console.log(`Failed to escape ${props.value}`);
      }
      q = encodeURIComponent(
        `${props.field ? props.field + ":" : ""}"${value}"`
      );
      break;
  }
  return (
    <A href={`/events?q=${q}`} class={props.class}>
      {props.children || props.value}
    </A>
  );
}

export function RawSearchLink(props: { children: any; q: string }) {
  const encoded = encodeURIComponent(props.q);
  return <A href={`/events?q=${encoded}`}>{props.children}</A>;
}
