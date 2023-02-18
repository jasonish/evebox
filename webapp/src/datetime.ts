// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import relativeTime from "dayjs/plugin/relativeTime";
import duration, { DurationUnitType } from "dayjs/plugin/duration";
import dayjs from "dayjs";

dayjs.extend(relativeTime);
dayjs.extend(duration);

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

// Return sane histogram intervals/bucket sizes for a given time range.
export function getInterval(rangeSeconds: number | undefined): string {
  let bounds = [
    { seconds: 60, interval: "1s" },
    { seconds: 3600 * 1, interval: "1m" },
    { seconds: 3600 * 3, interval: "2m" },
    { seconds: 3600 * 6, interval: "3m" },
    { seconds: 3600 * 12, interval: "5m" },
    { seconds: 3600 * 24, interval: "15m" },
    { seconds: 3600 * 24 * 3, interval: "1h" },
    { seconds: 3600 * 24 * 7, interval: "3h" },
  ];

  if (!rangeSeconds) {
    return "1d";
  } else {
    for (const bound of bounds) {
      if (rangeSeconds <= bound.seconds) {
        return bound.interval;
      }
    }
    return "1d";
  }
}
