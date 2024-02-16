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
