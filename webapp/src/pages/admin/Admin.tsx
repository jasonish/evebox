// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createStore } from "solid-js/store";
import { Top } from "../../Top";
import * as api from "../../api";
import { Show, createEffect, createMemo, createResource } from "solid-js";
import { serverConfig } from "../../config";

interface AutoArchiveSettings {
  enabled: boolean;
  value: number;
}

async function fetchAutoArchiveSettings(): Promise<AutoArchiveSettings> {
  const json = await api.API.getJson("api/admin/kv/config");
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

interface RetentionSettings {
  enabled: boolean;
  value: number;
}

async function fetchRetentionSettings(): Promise<AutoArchiveSettings> {
  const json = await api.API.getJson("api/admin/kv/config");
  const config = json["config.retention"];
  if (config) {
    return config;
  } else {
    return defaultRetentionSettings();
  }
}

function defaultRetentionSettings(): RetentionSettings {
  return {
    enabled: false,
    value: 365,
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

  const [retentionSettings, { refetch: refetchRetentionSettings }] =
    createResource<RetentionSettings>(fetchRetentionSettings);

  const [localRetentionSettings, setLocalRetentionSettings] =
    createStore<RetentionSettings>(defaultRetentionSettings());

  createEffect(() => {
    if (autoArchiveSettings()) {
      setLocalAutoArchiveSettings(autoArchiveSettings()!);
    }
  });

  createEffect(() => {
    if (retentionSettings()) {
      setLocalRetentionSettings(retentionSettings()!);
    }
  });

  const archiveSettingsModified = createMemo(() => {
    return (
      JSON.stringify(localAutoArchiveSettings) !=
      JSON.stringify(autoArchiveSettings.latest)
    );
  });

  const retentionSettingsModified = createMemo(() => {
    return (
      JSON.stringify(localRetentionSettings) !=
      JSON.stringify(retentionSettings.latest)
    );
  });

  const saveAutoArchiveSettings = async () => {
    await api.API.postJson(
      "api/admin/kv/config/config.autoarchive",
      localAutoArchiveSettings,
    );
    refetchAutoArchiveSettings();
  };

  const saveRetentionSettings = async () => {
    await api.API.postJson(
      "api/admin/kv/config/config.retention",
      localRetentionSettings,
    );
    refetchRetentionSettings();
  };

  const updateJa4Db = async (e: any) => {
    e.preventDefault();
    try {
      setState("ja4", { updating: true });
      await api.post("api/admin/update/ja4db");
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

  const links = () => {
    let links = [<>[</>];
    if (serverConfig?.datastore === "elasticsearch") {
      links.push(<a href="/admin/elastic">Elasticsearch</a>);
      links.push(<> | </>);
    }
    links.push(<a href="/admin/filters">Filters</a>);
    links.push(<>]</>);
    return links;
  };

  return (
    <>
      <Top />
      <div class="container mt-2">
        <div class="row">
          <div class="col">
            <span class="float-end">{links()}</span>
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

        {/* Auto archive. */}
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
                        onInput={(e) => {
                          setLocalAutoArchiveSettings("value", +e.target.value);
                        }}
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
                            autoArchiveSettings.latest!,
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

        {/* Retention settings. */}
        <div class="row mt-2">
          <div class="col">
            <div class="card">
              <div class="card-body">
                <Show when={serverConfig?.datastore === "elasticsearch"}>
                  <div class="row mt-2">
                    <div class="col">
                      Warning: Do not enable if you have Elasticsearch ILM
                      policies managing your indices.
                    </div>
                  </div>
                </Show>
                <Show when={serverConfig?.datastore === "sqlite"}>
                  <div class="row mt-2">
                    <div class="col">
                      Warning: This setting will not be effective if age
                      retention is set in the configuration file.
                    </div>
                  </div>
                </Show>
                <div class="row mt-2">
                  <label class="col col-form-label">
                    <div class="form-check form-switch">
                      <input
                        class="form-check-input"
                        type="checkbox"
                        role="switch"
                        checked={localRetentionSettings.enabled}
                        onChange={(e) => {
                          setLocalRetentionSettings({
                            enabled: e.target.checked,
                          });
                        }}
                      />
                      <label class="form-check-label">
                        <Show
                          when={serverConfig?.datastore === "elasticsearch"}
                        >
                          Delete indices older than:
                        </Show>
                        <Show when={serverConfig?.datastore === "sqlite"}>
                          Delete events older than:
                        </Show>
                      </label>
                    </div>
                  </label>
                  <div class="col">
                    <div class="input-group">
                      <input
                        type="number"
                        class="form-control"
                        value={localRetentionSettings.value}
                        onInput={(e) => {
                          setLocalRetentionSettings("value", +e.target.value);
                        }}
                        onChange={(e) => {
                          setLocalRetentionSettings("value", +e.target.value);
                        }}
                      />
                      <span class="input-group-text">Days</span>
                    </div>
                  </div>
                  <div class="col text-end">
                    <Show when={retentionSettingsModified()}>
                      <button
                        class="btn btn-success me-2"
                        onClick={() => {
                          saveRetentionSettings();
                        }}
                      >
                        Save
                      </button>
                      <button
                        class="btn btn-danger"
                        onClick={() => {
                          setLocalAutoArchiveSettings(
                            retentionSettings.latest!,
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
