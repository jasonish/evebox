// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal, onCleanup } from "solid-js";
import { API, fetchAgg } from "../api";
import { RefreshButton } from "../common/RefreshButton";
import { Chart, ChartConfiguration } from "chart.js";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { loadingTracker } from "../util";
import { CountValueDataTable } from "../components/CountValueDataTable";
import { StatCard } from "../components/StatCard";
import { Colors } from "../common/colors";
import { createStore } from "solid-js/store";
import type { SetStoreFunction } from "solid-js/store";
import dayjs from "dayjs";

interface CountValueRow {
  count: number;
  key: any;
}

interface Model {
  rows: CountValueRow[];
  loading: boolean;
  timestamp: null | dayjs.Dayjs;
}

function defaultModel(): Model {
  return {
    rows: [],
    loading: false,
    timestamp: null,
  };
}

interface Stats {
  anomalies: number | null;
  events: number | null;
  eventsCapped: boolean;
  sources: number | null;
  sourcesCapped: boolean;
}

function defaultStats(): Stats {
  return {
    anomalies: null,
    events: null,
    eventsCapped: false,
    sources: null,
    sourcesCapped: false,
  };
}

// Distinct event and source counts are exact until these aggregation
// row limits are hit, then shown as a lower bound.
const EVENT_LIMIT = 100;
const SOURCE_LIMIT = 500;

export function AnomaliesDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [events, setEvents] = createStore<Model>(defaultModel());

  const [appProtos, setAppProtos] = createStore<Model>(defaultModel());

  const [types, setTypes] = createStore<Model>(defaultModel());

  const [layers, setLayers] = createStore<Model>(defaultModel());

  const [topSources, setTopSources] = createStore<Model>(defaultModel());

  const [topDests, setTopDests] = createStore<Model>(defaultModel());

  const [stats, setStats] = createStore<Stats>(defaultStats());

  let histogram: any = undefined;

  onCleanup(() => {
    API.cancelAllSse();
  });

  createEffect(() => {
    refresh();
  });

  function buildChart(response: any) {
    const dataValues: number[] = [];
    const dataLabels: number[] = [];
    response.data.forEach((e: any) => {
      dataValues.push(e.count);
      dataLabels.push(e.time);
    });

    const ctx = (
      document.getElementById("histogram") as HTMLCanvasElement
    ).getContext("2d") as CanvasRenderingContext2D;

    const config: ChartConfiguration = {
      type: "bar",
      data: {
        labels: dataLabels,
        datasets: [
          {
            data: dataValues,
            backgroundColor: Colors[0],
            borderColor: Colors[0],
          },
        ],
      },
      options: {
        plugins: {
          title: {
            display: true,
            text: "Anomalies Over Time",
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
    if (histogram) {
      histogram.destroy();
    }
    histogram = new Chart(ctx, config);
  }

  function refresh() {
    setVersion((version) => version + 1);
    const timeRange = TIME_RANGE();

    let queryString = [];
    if (searchParams.q) {
      queryString.push(searchParams.q);
    }
    if (searchParams.sensor) {
      queryString.push(`host:${searchParams.sensor}`);
    }

    API.histogramTime({
      time_range: timeRange,
      event_type: "anomaly",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      buildChart(response);
    });

    // Stat cards. Every anomaly event carries anomaly.type, so the
    // total comes from summing the type aggregation.
    setStats(defaultStats());
    loadingTracker(setLoading, async () => {
      const request = {
        order: "desc" as const,
        time_range: timeRange,
      };
      const [typeRows, eventRows, sourceRows] = await Promise.all([
        fetchAgg({
          ...request,
          size: 100,
          field: "anomaly.type",
          q: [...queryString, "event_type:anomaly"].join(" "),
        }),
        fetchAgg({
          ...request,
          size: EVENT_LIMIT,
          field: "anomaly.event",
          q: [...queryString, "event_type:anomaly"].join(" "),
        }),
        fetchAgg({
          ...request,
          size: SOURCE_LIMIT,
          field: "src_ip",
          q: [...queryString, "event_type:anomaly"].join(" "),
        }),
      ]);
      setStats({
        anomalies: typeRows.rows.reduce((acc, row) => acc + row.count, 0),
        events: eventRows.rows.length,
        eventsCapped: eventRows.rows.length >= EVENT_LIMIT,
        sources: sourceRows.rows.length,
        sourcesCapped: sourceRows.rows.length >= SOURCE_LIMIT,
      });
    });

    let loaders: {
      field: string;
      q: string;
      order: "desc" | "asc";
      setter: SetStoreFunction<Model>;
    }[] = [
      // Anomaly events.
      {
        field: "anomaly.event",
        q: "event_type:anomaly",
        order: "desc",
        setter: setEvents,
      },
      // Affected application protocols.
      {
        field: "anomaly.app_proto",
        q: "event_type:anomaly",
        order: "desc",
        setter: setAppProtos,
      },
      // Anomaly types.
      {
        field: "anomaly.type",
        q: "event_type:anomaly",
        order: "desc",
        setter: setTypes,
      },
      // Anomaly layers.
      {
        field: "anomaly.layer",
        q: "event_type:anomaly",
        order: "desc",
        setter: setLayers,
      },
      // Top anomaly sources.
      {
        field: "src_ip",
        q: "event_type:anomaly",
        order: "desc",
        setter: setTopSources,
      },
      // Top anomaly destinations.
      {
        field: "dest_ip",
        q: "event_type:anomaly",
        order: "desc",
        setter: setTopDests,
      },
    ];

    for (const loader of loaders) {
      if (loader.setter) {
        loader.setter("loading", true);
      }

      let q = [...queryString];
      if (loader.q) {
        q.push(loader.q);
      }

      let request = {
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
        if (loader.setter) {
          loader.setter("loading", false);
        }
      });
    }
  }

  const formatSuffix = (timestamp: dayjs.Dayjs | null) => {
    if (timestamp) {
      return `since ${timestamp.fromNow()}`;
    }
    return undefined;
  };

  const formatCount = (count: number | null) =>
    count === null ? null : count.toLocaleString();

  return (
    <>
      <Top />

      <div class="container-fluid">
        <div class="row mt-2">
          <div class="col">
            <form class="d-flex flex-wrap align-items-center gap-2">
              <div>
                <RefreshButton loading={loading()} refresh={refresh} />
              </div>
              <div class="d-inline-flex">
                <SensorSelect
                  selected={searchParams.sensor}
                  onchange={(sensor) => {
                    setSearchParams({ sensor: sensor });
                  }}
                />
              </div>
            </form>
          </div>

          <div class="col">
            <form
              class="input-group"
              onsubmit={(e) => {
                e.preventDefault();
                setSearchParams({ q: e.currentTarget.filter.value });
              }}
            >
              <input
                id="filter-input"
                type="text"
                class="form-control"
                name="filter"
                placeholder="Search..."
                value={searchParams.q || ""}
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
                onclick={() => {
                  setSearchParams({ q: undefined });
                }}
              >
                Clear
              </button>
            </form>
          </div>
        </div>

        <div class="row mt-2 g-2">
          <div class="col-6 col-lg">
            <StatCard value={formatCount(stats.anomalies)} label="Anomalies" />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={
                stats.eventsCapped
                  ? `${stats.events}+`
                  : formatCount(stats.events)
              }
              label="Event Types"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={
                stats.sourcesCapped
                  ? `${stats.sources}+`
                  : formatCount(stats.sources)
              }
              label="Sources"
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <canvas id="histogram" class="app-chart-alerts"></canvas>
          </div>
        </div>

        <div class="row mt-2">
          <div class="col">
            <CountValueDataTable
              title={"Anomaly Events"}
              label={"Event"}
              searchField="anomaly.event"
              rows={events.rows}
              loading={events.loading}
              suffix={formatSuffix(events.timestamp)}
            />
          </div>

          <div class="col">
            <CountValueDataTable
              title={"Affected Application Protocols"}
              label={"Protocol"}
              searchField="anomaly.app_proto"
              tooltip={
                "Application protocol affected by the anomaly. Only application layer anomalies carry a protocol."
              }
              rows={appProtos.rows}
              loading={appProtos.loading}
              suffix={formatSuffix(appProtos.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Anomaly Types"}
              label={"Type"}
              searchField="anomaly.type"
              tooltip={
                "applayer: application layer parser anomalies. decode: packet decoder anomalies. stream: TCP stream anomalies."
              }
              rows={types.rows}
              loading={types.loading}
              suffix={formatSuffix(types.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Anomaly Layers"}
              label={"Layer"}
              searchField="anomaly.layer"
              rows={layers.rows}
              loading={layers.loading}
              suffix={formatSuffix(layers.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top Anomaly Sources"}
              label={"Address"}
              searchField={"@ip"}
              rows={topSources.rows}
              loading={topSources.loading}
              suffix={formatSuffix(topSources.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top Anomaly Destinations"}
              label={"Address"}
              searchField={"@ip"}
              rows={topDests.rows}
              loading={topDests.loading}
              suffix={formatSuffix(topDests.timestamp)}
            />
          </div>
        </div>
      </div>
    </>
  );
}
