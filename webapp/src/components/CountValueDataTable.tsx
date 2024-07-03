// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Accessor, For, Show } from "solid-js";
import { SearchLink } from "../common/SearchLink";

// Creates a table where the first column is a count, and the second
// column is value.
//
// TODO: Loading prop so we can display loading instead of no data.
export function CountValueDataTable(props: {
  title: string;
  label: string;
  searchField?: string;
  loading?: boolean;
  rows: { count: number; key: any }[];
}) {
  function searchLink(value: any) {
    if (props.searchField) {
      return (
        <SearchLink value={value} field={props.searchField}>
          {value}
        </SearchLink>
      );
    } else {
      return <SearchLink value={value}>{value}</SearchLink>;
    }
  }

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
