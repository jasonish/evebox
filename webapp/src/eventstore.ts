// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { EventWrapper } from "./types";
import { createMutable } from "solid-js/store";

export interface EventStore {
  events: EventWrapper[];
  active: EventWrapper | null;
  viewOffset: number;
  cursor: number;
  setActive: (active: EventWrapper) => void;
  reset: (events?: EventWrapper[]) => void;
}

export const eventStore = createMutable<EventStore>({
  events: [],
  active: null,
  viewOffset: 0,
  cursor: 0,

  setActive(active: EventWrapper) {
    this.active = active;
  },

  reset(events: EventWrapper[] = []) {
    this.events = events;
    this.active = null;
    this.viewOffset = 0;
    this.cursor = 0;
  },
});
