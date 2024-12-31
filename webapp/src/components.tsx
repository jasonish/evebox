// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For, Show } from "solid-js";
import { SearchLink } from "./common/SearchLink";
import { parse_timestamp } from "./datetime";
import { EventSource } from "./types";
import { formatAddress } from "./formatters";
import { BiDashCircle, BiFilter, BiPlusCircle } from "./icons";

// Creates a table where the first column is a count, and the second
// column is value.
export function CountValueDataTable(props: {
  title: string;
  label: string;
  searchField?: string;
  loading?: boolean;
  rows: { count: number; key: any }[];
}) {
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

  return (
    <>
      <div class="card app-count-value-data-table">
        <div class="card-header d-flex">
          {props.title}
          <Show when={props.loading !== undefined && props.loading}>
            {/* Loader in a button for placement reason's. */}
            <button
              class="btn ms-auto"
              type="button"
              disabled
              style="border: 0; padding: 0;"
            >
              <span
                class="spinner-border spinner-border-sm"
                aria-hidden="true"
              ></span>
              <span class="visually-hidden" role="status">
                Loading...
              </span>
            </button>
          </Show>
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

export function FilterStrip(props: { filters: any; setFilters: any }) {
  const removeFilter = (filter: any) => {
    props.setFilters((filters: any[]) =>
      filters.filter((f: any) => f !== filter)
    );
  };

  const invertFilter = (filter: string) => {
    props.setFilters((filters: any[]) => {
      const i = filters.indexOf(filter);
      if (filters[i].startsWith("-")) {
        filters[i] = filters[i].slice(1);
      } else {
        filters[i] = `-${filters[i]}`;
      }
      return [...filters];
    });
  };

  const buttonClass = (filter: string) => {
    if (filter.startsWith("-")) {
      return "filter-button-out";
    } else {
      return "filter-button-for";
    }
  };

  const isExclude = (filter: string) => {
    return filter.startsWith("-");
  };

  const isInclude = (filter: string) => {
    if (!filter.startsWith("@") && !filter.startsWith("-")) {
      return true;
    }
    return false;
  };

  return (
    <>
      <div class="row">
        <div class="col">
          <button
            class="btn btn-sm btn-secondary mt-2 me-1"
            onClick={() => props.setFilters([])}
          >
            Clear
          </button>
          <For each={props.filters()}>
            {(filter) => {
              return (
                <>
                  <div class="btn-group btn-group-sm mt-2 me-1" role="group">
                    <button
                      type="button"
                      class={"btn " + buttonClass(filter)}
                      data-bs-toggle="dropdown"
                    >
                      {filter}
                    </button>

                    <ul class="dropdown-menu">
                      <Show when={isExclude(filter)}>
                        <li>
                          <a
                            class="dropdown-item"
                            onClick={() => invertFilter(filter)}
                          >
                            Include results
                          </a>
                        </li>
                      </Show>

                      <Show when={isInclude(filter)}>
                        <li>
                          <a
                            class="dropdown-item"
                            onClick={() => invertFilter(filter)}
                          >
                            Exclude results
                          </a>
                        </li>
                      </Show>
                    </ul>

                    <button
                      type="button"
                      class={"btn " + buttonClass(filter)}
                      onClick={() => {
                        removeFilter(filter);
                      }}
                    >
                      X
                    </button>
                  </div>
                </>
              );
            }}
          </For>
        </div>
      </div>
    </>
  );
}

export function TimestampCell(props: {
  timestamp: string;
  addFilter?: (what: string, op: string, value: string) => void;
}) {
  let timestamp = parse_timestamp(props.timestamp);
  let formatted = timestamp.format("YYYY-MM-DD HH:mm:ss");
  return (
    <div title={props.timestamp}>
      {timestamp.format("YYYY-MM-DD HH:mm:ss")}
      <br />
      <span class={"small"}>{timestamp.fromNow()}</span>{" "}
      <Show when={props.addFilter}>
        <span class="dropdown" onclick={(e) => e.stopPropagation()}>
          <span data-bs-toggle="dropdown">
            <BiFilter />
          </span>
          <ul class="dropdown-menu">
            <li>
              <a
                class="dropdown-item"
                onClick={() => {
                  props.addFilter!("@from", "", props.timestamp);
                }}
              >
                Filter for from {formatted}
              </a>
            </li>
            <li>
              <a
                class="dropdown-item"
                onClick={() => {
                  props.addFilter!("@to", "", props.timestamp);
                }}
              >
                Filter for to {formatted}
              </a>
            </li>
          </ul>
        </span>
      </Show>
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
