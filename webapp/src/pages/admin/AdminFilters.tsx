// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  For,
  Show,
  Suspense,
  createEffect,
  createResource,
  createSignal,
} from "solid-js";
import { Top } from "../../Top";
import { API } from "../../api";

export function AdminFilters() {
  const [filters0, setFilters] = createSignal<any[]>([]);
  const [filters, { refetch }] = createResource(API.fetchFilters);
  let filterRef: any = undefined;

  createEffect(() => {
    setFilters(filters());
  });

  const onFilterChange = () => {
    const filterString = filterRef?.value;
    if (!filterString) {
      setFilters(filters);
      return;
    }

    setFilters(
      filters().filter((a: any) => {
        return JSON.stringify(a).indexOf(filterString) > -1;
      })
    );
  };

  const deleteFilter = (id: number) => {
    API.deleteFilter(id).then(() => {
      refetch();
    });
  };

  return (
    <>
      <Top />
      <div class="container mt-2">
        <div class="row">
          <div class="col">
            <h2>Auto Archive Filters</h2>
          </div>
          <div class="col">
            <input
              ref={filterRef}
              type="text"
              class="form-control"
              placeholder="Filter..."
              oninput={onFilterChange}
            />
          </div>
        </div>
        <Suspense>
          <div class="card">
            <div class="card-body">
              <div class="row">
                <div class="col fw-bold">Sensor</div>
                <div class="col fw-bold">Source IP</div>
                <div class="col fw-bold">Destination IP</div>
                <div class="col fw-bold">Signature ID</div>
                <div class="col fw-bold"></div>
              </div>
            </div>
          </div>
          <For each={filters0()}>
            {(filter) => {
              return (
                <>
                  <div class="card mt-1">
                    <div class="card">
                      <div class="card-body">
                        <div class="row">
                          <div class="col">{filter.filter.sensor || "*"}</div>
                          <div class="col">{filter.filter.src_ip || "*"}</div>
                          <div class="col">{filter.filter.dest_ip || "*"}</div>
                          <div class="col">{filter.filter.signature_id}</div>
                          <div class="col text-end">
                            <button
                              class="btn btn-warning"
                              onClick={() => deleteFilter(filter.id)}
                            >
                              Delete
                            </button>
                          </div>
                          <Show when={filter.comment}>
                            <div class="col-12">
                              <span class="fw-bold">Comment: </span>
                              {filter.comment}
                            </div>
                          </Show>
                        </div>
                      </div>
                    </div>
                  </div>
                </>
              );
            }}
          </For>
        </Suspense>
      </div>
    </>
  );
}
