// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createSignal } from "solid-js";

export const [PREFS, SET_PREFS] = createSignal<ClientPreferences>(
  getClientPreferences()
);

export interface ClientPreferences {
  timestamp_format?: "utc" | "local";
}

export function createDefaultClientPreferences(): ClientPreferences {
  return {
    timestamp_format: "local",
  };
}

export function getClientPreferences(): ClientPreferences {
  let prefs: any = localStorage.getItem("clientPreferences");
  if (!prefs) {
    console.log("Did not find localStorage clientPreferences, return defaults");
    return createDefaultClientPreferences();
  }
  try {
    prefs = JSON.parse(prefs) as ClientPreferences;
    // Merge in defaults.
    prefs = { ...createDefaultClientPreferences(), ...prefs };
    return prefs;
  } catch (e) {
    return createDefaultClientPreferences();
  }
}

export function saveClientPreferences(prefs: ClientPreferences) {
  localStorage.setItem("clientPreferences", JSON.stringify(prefs));
  SET_PREFS(prefs);
}
