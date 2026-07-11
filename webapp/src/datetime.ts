// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import relativeTime from "dayjs/plugin/relativeTime";
import duration, { DurationUnitType } from "dayjs/plugin/duration";
import utc from "dayjs/plugin/utc";
import dayjs from "dayjs";

dayjs.extend(relativeTime);
dayjs.extend(duration);
dayjs.extend(utc);

export function parse_timestamp(timestamp: string): dayjs.Dayjs {
  return dayjs(timestamp);
}

export function parse_timerange(timerange: string): undefined | number {
  const match = timerange.match(/(\d+)(.*)/);
  if (match && match[1] && match[2]) {
    const value = match[1];
    const units = match[2] as DurationUnitType;
    const duration = dayjs.duration(+value, units);
    return duration.as("s");
  }
}

export function get_duration(value: number, unit: DurationUnitType = "s") {
  return dayjs.duration(value, unit);
}

export function get_timezone_offset_str(): string {
  return dayjs().format("ZZ");
}

// Format a timestamp (or now, when omitted) as the local wall-clock
// value an <input type="datetime-local"> expects: "YYYY-MM-DDTHH:mm:ss"
// with no timezone.
export function to_datetime_local_input(timestamp?: string): string {
  const d = timestamp ? dayjs(timestamp) : dayjs();
  return d.format("YYYY-MM-DDTHH:mm:ss");
}

// Convert a datetime-local input value (local wall-clock, no zone) into
// an absolute RFC3339/ISO-8601 timestamp for the API.
export function datetime_local_to_rfc3339(value: string): string {
  return dayjs(value).toISOString();
}

// Whether a datetime-local input value is a parseable timestamp. Guards
// callers of `datetime_local_to_rfc3339`, which throws on an empty or
// invalid value.
export function is_valid_datetime_local(value: string): boolean {
  return value.trim() !== "" && dayjs(value).isValid();
}
