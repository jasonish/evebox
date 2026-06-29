// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { ConfigResponse } from "./api";

export let serverConfig: ConfigResponse | null = null;

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
  serverConfig = config;
}

/// The display name of the active Elasticsearch distribution: "OpenSearch" when
/// the backend really is OpenSearch, otherwise "Elasticsearch".
export function distributionName(): string {
  return serverConfig?.distribution === "opensearch"
    ? "OpenSearch"
    : "Elasticsearch";
}
