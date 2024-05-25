// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For, Show } from "solid-js";
import { SearchLink } from "../common/SearchLink";

// Creates a table where the first column is a count, and the second
// column is value.
export function CountValueDataTable(props: {
  title: string;
  label: string;
  searchField?: string;
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
        <div class="card-header">{props.title}</div>
        <div class="card-body p-0">
          <Show when={props.rows.length == 0}></Show>
          <Show when={props.rows.length > 0}>
            <table class="table mb-0">
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
                      <td>{searchLink(row.key)}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </Show>
        </div>
      </div>
    </>
  );
}
