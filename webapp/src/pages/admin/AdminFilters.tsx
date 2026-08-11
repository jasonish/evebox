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
import { API } from "../../api";
import { AdminPageHeader } from "./AdminLayout";

export function AdminFilters() {
  const [filters0, setFilters] = createSignal<any[]>([]);
  const [filters, { refetch }] = createResource(API.fetchFilters);
  const [showRaw, setShowRaw] = createSignal(false);
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
      }),
    );
  };

  const deleteFilter = (id: number) => {
    API.deleteFilter(id).then(() => {
      refetch();
    });
  };

  return (
    <>
      <AdminPageHeader
        title="Auto-Archive Filters"
        subtitle="Matching alerts are archived automatically as they arrive."
      >
        <div class="d-flex flex-wrap align-items-center gap-3">
          <label class="form-check-label d-flex align-items-center gap-2">
            <input
              class="form-check-input mt-0"
              type="checkbox"
              checked={showRaw()}
              onChange={(e) => setShowRaw(e.target.checked)}
            />
            Show raw JSON
          </label>
          <input
            ref={filterRef}
            type="text"
            class="form-control w-auto"
            placeholder="Filter..."
            oninput={onFilterChange}
          />
        </div>
      </AdminPageHeader>
      <Suspense>
        <For each={filters0()}>
          {(filter) => {
            return (
              <>
                <div class="card mt-1">
                  <div class="card">
                    <div class="card-body">
                      <div class="row">
                        <div class="col">
                          <div class="fw-bold">Conditions:</div>
                          <For each={filter.filter.conditions}>
                            {(condition) => (
                              <div class="ms-3 app-break-anywhere">
                                <code>{condition.field}</code>{" "}
                                {condition.op === "eq" ? "=" : condition.op}{" "}
                                <code>{JSON.stringify(condition.value)}</code>
                              </div>
                            )}
                          </For>
                        </div>
                        <div class="col-auto text-end">
                          <button
                            class="btn btn-warning"
                            onClick={() => deleteFilter(filter.id)}
                          >
                            Delete
                          </button>
                        </div>
                        <Show when={filter.comment}>
                          <div class="col-12 mt-2">
                            <span class="fw-bold">Comment: </span>
                            {filter.comment}
                          </div>
                        </Show>
                        <Show when={showRaw()}>
                          <div class="col-12 mt-2">
                            <pre class="bg-body-tertiary border rounded p-2 mb-0 app-break-anywhere app-pre-wrap">
                              {JSON.stringify(filter.filter, undefined, 2)}
                            </pre>
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
    </>
  );
}
