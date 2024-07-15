// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createStore } from "solid-js/store";
import { Top } from "../../Top";
import * as api from "../../api";
import { Show } from "solid-js";

export function Admin() {
  const [state, setState] = createStore({
    ja4: {
      updating: false,
      success: false,
      failed: false,
    },
  });

  const updateJa4Db = async () => {
    try {
      setState("ja4", { updating: true });
      await api.post("/api/admin/update/ja4db");
      setState("ja4", {
        success: true,
        failed: false,
      });
    } catch (e) {
      setState("ja4", {
        success: false,
        failed: true,
      });
    } finally {
      setState("ja4", { updating: false });
    }
  };

  return (
    <>
      <Top />
      <div class="container mt-2">
        <div class="row">
          <div class="col">
            <div class="card">
              <form class="card-body d-flex justify-content-between align-items-center">
                Update JA4db:
                <div>
                  <Show when={state.ja4.updating}>
                    <div class="badge text-bg-primary me-2">Updating</div>
                  </Show>
                  <Show when={state.ja4.success}>
                    <div class="badge text-bg-success me-2">
                      Update successful
                    </div>
                  </Show>
                  <Show when={state.ja4.failed}>
                    <div class="badge text-bg-danger me-2">Update failed</div>
                  </Show>
                  <button class="btn btn-primary" onClick={updateJa4Db}>
                    Update
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
