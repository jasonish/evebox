// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For, Suspense, createResource } from "solid-js";
import { API } from "../../api";
import { Top } from "../../Top";
import { addError, addNotification } from "../../Notifications";

async function fetchIndices() {
  const indices = await API.getJson("api/admin/elastic/indices");
  return indices;
}

export function AdminElastic() {
  const [indices, { refetch: refetchIndices }] = createResource(fetchIndices);

  const deleteIndex = (name: string) => {
    API.doDelete(`api/admin/elastic/index/${name}`)
      .then((response) => {
        console.log(response);
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
        <h1>Elasticsearch Index Management</h1>
        <Suspense>
          <table class="table table-striped">
            <thead>
              <tr>
                <th>Name</th>
                <th>Doc Count</th>
                <th>Store Size</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              <For each={indices()}>
                {(e) => {
                  return (
                    <>
                      <tr>
                        <td class="align-middle">{e.name}</td>
                        <td class="align-middle">{e.doc_count}</td>
                        <td class="align-middle">{e.store_size}</td>
                        <td class="align-middle text-end">
                          <button
                            class="btn btn-danger"
                            onClick={() => {
                              deleteIndex(e.name);
                            }}
                          >
                            Delete
                          </button>
                        </td>
                      </tr>
                    </>
                  );
                }}
              </For>
            </tbody>
          </table>
        </Suspense>
      </div>
    </>
  );
}
