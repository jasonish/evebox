// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createStore } from "solid-js/store";
import { Top } from "../../Top";
import * as api from "../../api";
import { Show, createEffect, createMemo, createResource } from "solid-js";

interface AutoArchiveSettings {
  enabled: boolean;
  value: number;
}

async function fetchAutoArchiveSettings(): Promise<AutoArchiveSettings> {
  const json = await api.API.getJson("/api/admin/kv/config");
  const config = json["config.autoarchive"];
  if (config) {
    return config;
  } else {
    return defaultAutoArchiveSettings();
  }
}

function defaultAutoArchiveSettings(): AutoArchiveSettings {
  return {
    enabled: false,
    value: 7,
  };
}

export function Admin() {
  const [state, setState] = createStore({
    ja4: {
      updating: false,
      success: false,
      failed: false,
    },
  });

  const [autoArchiveSettings, { refetch: refetchAutoArchiveSettings }] =
    createResource<AutoArchiveSettings>(fetchAutoArchiveSettings);
  const [localAutoArchiveSettings, setLocalAutoArchiveSettings] =
    createStore<AutoArchiveSettings>(defaultAutoArchiveSettings());

  createEffect(() => {
    if (autoArchiveSettings()) {
      setLocalAutoArchiveSettings(autoArchiveSettings()!);
    }
  });

  const archiveSettingsModified = createMemo(() => {
    return (
      JSON.stringify(localAutoArchiveSettings) !=
      JSON.stringify(autoArchiveSettings.latest)
    );
  });

  const saveAutoArchiveSettings = async () => {
    await api.API.postJson(
      "/api/admin/kv/config/config.autoarchive",
      localAutoArchiveSettings
    );
    refetchAutoArchiveSettings();
  };

  const updateJa4Db = async (e: any) => {
    e.preventDefault();
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
            <span class="float-end">
              [ <a href="/admin/filters">Filters</a> ]
            </span>
          </div>
        </div>

        <div class="row mt-2">
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

        <div class="row mt-2">
          <div class="col">
            <div class="card">
              <div class="card-body">
                <div class="row">
                  <label class="col col-form-label">
                    <div class="form-check form-switch">
                      <input
                        class="form-check-input"
                        type="checkbox"
                        role="switch"
                        checked={localAutoArchiveSettings.enabled}
                        onChange={(e) => {
                          setLocalAutoArchiveSettings({
                            enabled: e.target.checked,
                          });
                        }}
                      />
                      <label class="form-check-label">
                        Auto-archive events older than:
                      </label>
                    </div>
                  </label>
                  <div class="col">
                    <div class="input-group">
                      <input
                        type="number"
                        class="form-control"
                        value={localAutoArchiveSettings.value}
                        onChange={(e) => {
                          setLocalAutoArchiveSettings("value", +e.target.value);
                        }}
                      />
                      <span class="input-group-text">Days</span>
                    </div>
                  </div>
                  <div class="col text-end">
                    <Show when={archiveSettingsModified()}>
                      <button
                        class="btn btn-success me-2"
                        onClick={() => {
                          saveAutoArchiveSettings();
                        }}
                      >
                        Save
                      </button>
                      <button
                        class="btn btn-danger"
                        onClick={() => {
                          setLocalAutoArchiveSettings(
                            autoArchiveSettings.latest!
                          );
                        }}
                      >
                        Reset
                      </button>
                    </Show>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
