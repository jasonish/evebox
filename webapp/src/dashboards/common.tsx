// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Shared pieces of the protocol dashboards: table models, aggregation
// loaders, the events-over-time chart and the filter bar.

import { Accessor, Setter } from "solid-js";
import type { SetStoreFunction } from "solid-js/store";
import { Chart, ChartConfiguration } from "chart.js";
import dayjs from "dayjs";
import { API, AggResponse } from "../api";
import { RefreshButton } from "../common/RefreshButton";
import { SensorSelect } from "../common/SensorSelect";
import { Colors } from "../common/colors";
import { loadingTracker } from "../util";

export interface CountValueRow {
  count: number;
  key: any;
}

// The rows of a count/value table.
export interface Model {
  rows: CountValueRow[];
  loading: boolean;
  timestamp: null | dayjs.Dayjs;
}

export function defaultModel(): Model {
  return {
    rows: [],
    loading: false,
    timestamp: null,
  };
}

// Distinct counts are exact until this aggregation row limit is hit,
// then shown as a lower bound.
export const DISTINCT_LIMIT = 500;

export const formatSuffix = (timestamp: dayjs.Dayjs | null) =>
  timestamp ? `since ${timestamp.fromNow()}` : undefined;

export const formatCount = (count: number | null) =>
  count === null ? null : count.toLocaleString();

// A distinct count from the rows of an aggregation limited to
// DISTINCT_LIMIT rows.
export const formatDistinct = (count: number | null) =>
  count !== null && count >= DISTINCT_LIMIT ? `${count}+` : formatCount(count);

export function percentOf(
  count: number | null,
  total: number | null,
  of: string,
): string | undefined {
  if (count === null || !total) {
    return undefined;
  }
  const percent = (count / total) * 100;
  if (count > 0 && percent < 0.1) {
    return `<0.1% of ${of}`;
  }
  return `${percent.toFixed(1)}% of ${of}`;
}

// The sum of the counts of aggregation rows.
export const sumRows = (rows: CountValueRow[]) =>
  rows.reduce((acc, row) => acc + row.count, 0);

// Merge the rows of several aggregations, summing the counts of equal
// keys, and return the top rows.
export function mergeRows(
  responses: AggResponse[],
  limit: number = 10,
): CountValueRow[] {
  const merged: Map<any, number> = new Map();
  for (const response of responses) {
    for (const row of response.rows) {
      merged.set(row.key, (merged.get(row.key) || 0) + row.count);
    }
  }
  return [...merged.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((a, b) => b.count - a.count)
    .slice(0, limit);
}

// An aggregation streamed into a table.
export interface AggLoader {
  field: string;
  q: string;
  order: "desc" | "asc";
  setter: SetStoreFunction<Model>;
}

export function runAggLoaders(
  loaders: AggLoader[],
  queryString: string[],
  timeRange: string,
  version: Accessor<number>,
  setLoading: Setter<number>,
) {
  for (const loader of loaders) {
    loader.setter("loading", true);
    const q = [...queryString, loader.q].filter((q) => q);
    const request = {
      time_range: timeRange,
      field: loader.field,
      order: loader.order,
      q: q.length > 0 ? q.join(" ") : undefined,
    };
    loadingTracker(setLoading, () => {
      return API.getSseAgg(request, version, (data: any) => {
        if (data) {
          loader.setter("rows", data.rows);
          loader.setter("timestamp", dayjs(data.earliest_ts));
        }
      });
    }).finally(() => {
      loader.setter("loading", false);
    });
  }
}

// Draw an events-over-time bar chart into a canvas, replacing the
// previous chart.
export function buildHistogram(
  canvasId: string,
  title: string,
  response: { data: { count: number; time: number }[] },
  previous: Chart | undefined,
): Chart {
  const ctx = (
    document.getElementById(canvasId) as HTMLCanvasElement
  ).getContext("2d") as CanvasRenderingContext2D;

  const config: ChartConfiguration = {
    type: "bar",
    data: {
      labels: response.data.map((e) => e.time),
      datasets: [
        {
          data: response.data.map((e) => e.count),
          backgroundColor: Colors[0],
          borderColor: Colors[0],
        },
      ],
    },
    options: {
      plugins: {
        title: {
          display: true,
          text: title,
          padding: 0,
        },
        legend: {
          display: false,
        },
      },
      scales: {
        x: {
          type: "time",
          ticks: {
            source: "auto",
          },
        },
      },
    },
  };
  if (previous) {
    previous.destroy();
  }
  return new Chart(ctx, config);
}

// The refresh button, sensor selector and search box above a dashboard.
export function DashboardFilters(props: {
  loading: number;
  refresh: () => void;
  sensor?: string;
  setSensor: (sensor: string | undefined) => void;
  q?: string;
  setQuery: (q: string | undefined) => void;
}) {
  return (
    <div class="row mt-2">
      <div class="col">
        <form class="d-flex flex-wrap align-items-center gap-2">
          <div>
            <RefreshButton loading={props.loading} refresh={props.refresh} />
          </div>
          <div class="d-inline-flex">
            <SensorSelect
              selected={props.sensor}
              onchange={(sensor) => props.setSensor(sensor)}
            />
          </div>
        </form>
      </div>

      <div class="col">
        <form
          class="input-group"
          onsubmit={(e) => {
            e.preventDefault();
            props.setQuery(e.currentTarget.filter.value);
          }}
        >
          <input
            id="filter-input"
            type="text"
            class="form-control"
            name="filter"
            placeholder="Search..."
            value={props.q || ""}
            onkeydown={(e) => {
              e.stopPropagation();
            }}
          />
          <button class="btn btn-secondary" type="submit">
            Apply
          </button>
          <button
            class="btn btn-secondary"
            type="button"
            onclick={() => props.setQuery(undefined)}
          >
            Clear
          </button>
        </form>
      </div>
    </div>
  );
}
