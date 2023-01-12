// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { EventWrapper } from "./types";

export enum Tag {
  Archived = "evebox.archived",
  Escalated = "evebox.escalated",
}

export function eventIsArchived(event: EventWrapper) {
  return event._source.tags && event._source.tags.indexOf(Tag.Archived) > -1;
}

function eventEnsureHasTags(event: EventWrapper) {
  if (!event._source.tags) {
    event._source.tags = [];
  }
}

export function eventSetArchived(event: EventWrapper) {
  if (!eventIsArchived(event)) {
    eventAddTag(event, Tag.Archived);
  }
}

export function eventSetEscalated(event: EventWrapper) {
  eventAddTag(event, Tag.Escalated);
}

export function eventAddTag(event: EventWrapper, tag: string) {
  eventEnsureHasTags(event);
  event._source.tags!.push(tag);
}
