// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For, JSXElement, Show, onMount } from "solid-js";
import { SearchLink } from "../common/SearchLink";
import { BiInfoCircle } from "../icons";
import { Tooltip } from "bootstrap";

// Creates a table where the first column is a count, and the second
// column is value.
export function CountValueDataTable(props: {
  title: string | JSXElement | (() => string | JSXElement);
  suffix?: string | JSXElement | (() => string | JSXElement);
  label: string;
  searchField?: string;
  loading?: boolean;
  rows: { count: number; key: any }[];
  tooltip?: string;
}) {
  let tooltipRef: HTMLSpanElement | undefined;

  onMount(() => {
    // Initialize tooltips when the component mounts
    if (tooltipRef && props.tooltip) {
      new Tooltip(tooltipRef, {
        trigger: "hover",
      });
    }
  });

  const searchLink = (value: any) => {
    if (props.searchField) {
      return (
        <SearchLink value={value} field={props.searchField}>
          {value}
        </SearchLink>
      );
    } else {
      return <SearchLink value={value}>{value}</SearchLink>;
    }
  };

  const title = () => {
    if (typeof props.title === "function") {
      return props.title();
    }
    return props.title;
  };

  const suffix = () => {
    if (typeof props.suffix === "function") {
      return props.suffix();
    }
    return props.suffix;
  };

  return (
    <>
      <div class="card app-count-value-data-table">
        <div class="card-header d-flex">
          <div class="d-flex align-items-center">
            {title()}
            <Show when={props.tooltip}>
              <span
                ref={tooltipRef}
                class="ms-1"
                data-bs-toggle="tooltip"
                data-bs-placement="top"
                title={props.tooltip}
              >
                <BiInfoCircle class="text-info" />
              </span>
            </Show>
          </div>
          <div class="ms-auto d-flex align-items-center">
            <Show when={props.loading !== undefined && props.loading}>
              {/* Loader for placement reason's. */}
              <div>
                <span class="me-2 small text-muted fst-italic">
                  {suffix()}{" "}
                </span>
                <span
                  class="spinner-border spinner-border-sm"
                  aria-hidden="true"
                ></span>
                <span class="visually-hidden" role="status">
                  Loading...
                </span>
              </div>
            </Show>
          </div>
        </div>
        <Show when={props.rows.length == 0}>
          <div class="card-body">No data</div>
        </Show>
        <Show when={props.rows.length > 0}>
          <div class="card-body p-0">
            <table class="table" style="margin-bottom: 3px;">
              <thead>
                <tr>
                  <th style={"width: 6em;"}>#</th>
                  <th>{props.label}</th>
                </tr>
              </thead>
              <tbody>
                <For each={props.rows}>
                  {(row) => (
                    <tr>
                      <td style={"width: 6em;"}>{row.count}</td>
                      <td class="force-wrap">{searchLink(row.key)}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </div>
    </>
  );
}
