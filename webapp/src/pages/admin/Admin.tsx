// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createStore } from "solid-js/store";
import * as api from "../../api";
import { Show, createEffect, createMemo, createResource } from "solid-js";
import { distributionName, serverConfig } from "../../config";
import { AdminPageHeader } from "./AdminLayout";

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

async function fetchRetentionSizeSettings(): Promise<AutoArchiveSettings> {
  const json = await api.API.getJson("api/admin/kv/config");
  const config = json["config.retention.size"];
  if (config) {
    return config;
  } else {
    return defaultRetentionSizeSettings();
  }
}

function defaultRetentionSettings(): RetentionSettings {
  return {
    enabled: false,
    value: 365,
  };
}

function defaultRetentionSizeSettings(): RetentionSettings {
  return {
    enabled: false,
    value: 20,
  };
}

export function Admin() {
  // Auto archive.
  const [autoArchiveSettings, { refetch: refetchAutoArchiveSettings }] =
    createResource<AutoArchiveSettings>(fetchAutoArchiveSettings);
  const [localAutoArchiveSettings, setLocalAutoArchiveSettings] =
    createStore<AutoArchiveSettings>(defaultAutoArchiveSettings());

  // Retention by age.
  const [retentionSettings, { refetch: refetchRetentionSettings }] =
    createResource<RetentionSettings>(fetchRetentionSettings);
  const [localRetentionSettings, setLocalRetentionSettings] =
    createStore<RetentionSettings>(defaultRetentionSettings());

  // Retention by size.
  const [retentionSizeSettings, { refetch: refetchRetentionSizeSettings }] =
    createResource<RetentionSettings>(fetchRetentionSizeSettings);
  const [localRetentionSizeSettings, setLocalRetentionSizeSettings] =
    createStore<RetentionSettings>(defaultRetentionSizeSettings());

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

  createEffect(() => {
    if (retentionSizeSettings()) {
      setLocalRetentionSizeSettings(retentionSizeSettings()!);
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

  const retentionSizeSettingsModified = createMemo(() => {
    return (
      JSON.stringify(localRetentionSizeSettings) !=
      JSON.stringify(retentionSizeSettings.latest)
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

  const saveRetentionSizeSettings = async () => {
    await api.API.postJson(
      "api/admin/kv/config/config.retention.size",
      localRetentionSizeSettings,
    );
    refetchRetentionSizeSettings();
  };

  return (
    <>
      <AdminPageHeader
        title="General"
        subtitle="Server maintenance and event retention."
      />

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

      {/* Retention by age. */}
      <div class="row mt-2">
        <div class="col">
          <div class="card">
            <div class="card-body">
              <Show when={serverConfig()?.datastore === "elasticsearch"}>
                <div class="row mt-2">
                  <div class="col">
                    Warning: Do not enable if you have {distributionName()}{" "}
                    {serverConfig()?.distribution === "opensearch"
                      ? "ISM"
                      : "ILM"}{" "}
                    policies managing your indices.
                  </div>
                </div>
              </Show>
              <Show when={serverConfig()?.datastore === "sqlite"}>
                <div class="row mt-2">
                  <div class="col">
                    Warning: This setting will not be effective if age retention
                    is set in the configuration file.
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
                        when={serverConfig()?.datastore === "elasticsearch"}
                      >
                        Delete indices older than:
                      </Show>
                      <Show when={serverConfig()?.datastore === "sqlite"}>
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
                        setLocalRetentionSettings(retentionSettings.latest!);
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

      {/* Retention by disk size. */}
      <Show when={serverConfig()?.datastore === "sqlite"}>
        <div class="row mt-2">
          <div class="col">
            <div class="card">
              <div class="card-body">
                <div class="row mt-2">
                  <div class="col">
                    Warning: This setting will not be effective if size
                    retention is set in the configuration file.
                  </div>
                </div>
                <div class="row mt-2">
                  <label class="col col-form-label">
                    <div class="form-check form-switch">
                      <input
                        class="form-check-input"
                        type="checkbox"
                        role="switch"
                        checked={localRetentionSizeSettings.enabled}
                        onChange={(e) => {
                          setLocalRetentionSizeSettings({
                            enabled: e.target.checked,
                          });
                        }}
                      />
                      <label class="form-check-label">
                        Limit event database to size:
                      </label>
                    </div>
                  </label>
                  <div class="col">
                    <div class="input-group">
                      <input
                        type="number"
                        class="form-control"
                        value={localRetentionSizeSettings.value}
                        onInput={(e) => {
                          setLocalRetentionSizeSettings(
                            "value",
                            +e.target.value,
                          );
                        }}
                        onChange={(e) => {
                          setLocalRetentionSizeSettings(
                            "value",
                            +e.target.value,
                          );
                        }}
                      />
                      <span class="input-group-text">Gigabytes</span>
                    </div>
                  </div>
                  <div class="col text-end">
                    <Show when={retentionSizeSettingsModified()}>
                      <button
                        class="btn btn-success me-2"
                        onClick={() => {
                          saveRetentionSizeSettings();
                        }}
                      >
                        Save
                      </button>
                      <button
                        class="btn btn-danger"
                        onClick={() => {
                          setLocalRetentionSizeSettings(
                            retentionSizeSettings.latest!,
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
      </Show>
    </>
  );
}
