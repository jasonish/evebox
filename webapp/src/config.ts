// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { ConfigResponse } from "./api";
import { createSignal } from "solid-js";

const [serverConfigValue, setServerConfigValue] =
  createSignal<ConfigResponse | null>(null);

export const serverConfig = serverConfigValue;

export interface EventServiceConfig {
  type: string;
  enabled: boolean;
  "event-types": string[];
  url: string;
  target: string;
  name: string;
  datastore: string;
}

export function serverConfigSet(config: ConfigResponse) {
  setServerConfigValue((current) =>
    current && JSON.stringify(current) === JSON.stringify(config)
      ? current
      : config,
  );
}

/// The display name of the active Elasticsearch distribution: "OpenSearch" when
/// the backend really is OpenSearch, otherwise "Elasticsearch".
export function distributionName(): string {
  return serverConfig()?.distribution === "opensearch"
    ? "OpenSearch"
    : "Elasticsearch";
}
