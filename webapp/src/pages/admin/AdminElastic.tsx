// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For, Suspense, createResource } from "solid-js";
import { API } from "../../api";
import { Top } from "../../Top";
import { addError, addNotification } from "../../Notifications";
import { distributionName } from "../../config";

interface IndexStats {
  name: string;
  doc_count: number;
  store_size: number;
}

interface IndexGroup {
  // The "YYYY.MM.DD" date the indices share, or "" when undated.
  date: string;
  label: string;
  indices: IndexStats[];
  docCount: number;
  storeSize: number;
}

const DATE_RE = /(\d{4}\.\d{2}\.\d{2})/;

async function fetchIndices(): Promise<IndexStats[]> {
  return await API.getJson("api/admin/elastic/indices");
}

// Human readable byte size (1024-based).
function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes)) {
    return "-";
  }
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const rounded = unit === 0 ? value : Math.round(value * 10) / 10;
  return `${rounded} ${units[unit]}`;
}

// Group indices by their date suffix so the daily event index and its companion
// stats index (e.g. logstash-2026.06.28 and logstash-stats-2026.06.28) appear
// together. Dated groups are sorted newest-first; undated indices go last.
function groupByDate(indices: IndexStats[]): IndexGroup[] {
  const groups = new Map<string, IndexGroup>();
  for (const index of indices) {
    const match = index.name.match(DATE_RE);
    const date = match ? match[1] : "";
    let group = groups.get(date);
    if (!group) {
      group = {
        date,
        label: date || "Undated",
        indices: [],
        docCount: 0,
        storeSize: 0,
      };
      groups.set(date, group);
    }
    group.indices.push(index);
    group.docCount += index.doc_count ?? 0;
    group.storeSize += index.store_size ?? 0;
  }
  for (const group of groups.values()) {
    group.indices.sort((a, b) => a.name.localeCompare(b.name));
  }
  return [...groups.values()].sort((a, b) => {
    if (a.date === "") return 1;
    if (b.date === "") return -1;
    return b.date.localeCompare(a.date);
  });
}

export function AdminElastic() {
  const [indices, { refetch: refetchIndices }] = createResource(fetchIndices);

  const groups = () => groupByDate(indices() ?? []);

  const deleteIndex = (name: string) => {
    API.doDelete(`api/admin/elastic/index/${name}`)
      .then(() => {
        addNotification(`Index ${name} has been deleted.`);
        refetchIndices();
      })
      .catch((error: any) => {
        addError(`error: ${error}`);
      });
  };

  return (
    <>
      <Top />
      <div class="container mt-2">
        <h1>{distributionName()} Index Management</h1>
        <Suspense>
          <table class="table table-striped">
            <thead>
              <tr>
                <th>Name</th>
                <th class="text-end">Doc Count</th>
                <th class="text-end">Store Size</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              <For each={groups()}>
                {(group) => (
                  <>
                    <tr class="table-secondary">
                      <th>{group.label}</th>
                      <th class="text-end">
                        {group.docCount.toLocaleString()}
                      </th>
                      <th class="text-end">{formatBytes(group.storeSize)}</th>
                      <th></th>
                    </tr>
                    <For each={group.indices}>
                      {(e) => (
                        <tr>
                          <td class="align-middle ps-4">{e.name}</td>
                          <td class="align-middle text-end">
                            {e.doc_count.toLocaleString()}
                          </td>
                          <td class="align-middle text-end">
                            {formatBytes(e.store_size)}
                          </td>
                          <td class="align-middle text-end">
                            <button
                              class="btn btn-danger btn-sm"
                              onClick={() => {
                                deleteIndex(e.name);
                              }}
                            >
                              Delete
                            </button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </>
                )}
              </For>
            </tbody>
          </table>
        </Suspense>
      </div>
    </>
  );
}
