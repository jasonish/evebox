// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { EventWrapper } from "./types";
import { createMutable } from "solid-js/store";

export interface EventStore {
  events: EventWrapper[];
  active: EventWrapper | null;
  viewOffset: number;
  viewPath: string | null;
  cursor: number;
  setActive: (active: EventWrapper, viewPath: string) => void;
  reset: (events?: EventWrapper[]) => void;
}

export const eventStore = createMutable<EventStore>({
  events: [],
  active: null,
  viewOffset: 0,
  viewPath: null,
  cursor: 0,

  setActive(active: EventWrapper, viewPath: string) {
    this.active = active;
    this.viewPath = viewPath;
  },

  reset(events: EventWrapper[] = []) {
    this.events = events;
    this.active = null;
    this.viewOffset = 0;
    this.viewPath = null;
    this.cursor = 0;
  },
});
