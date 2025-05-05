// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal, onCleanup } from "solid-js";
import { API } from "../api";
import { RefreshButton } from "../common/RefreshButton";
import { Chart, ChartConfiguration } from "chart.js";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { loadingTracker } from "../util";
import { CountValueDataTable } from "../components";
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

export function AlertsDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [mostAlerting, setMostAlerting] = createStore<Model>(defaultModel());

  const [leastAlerting, setLeastAlerting] = createStore<Model>(defaultModel());

  const [mostAlertingSource, setMostAlertingSource] =
    createStore<Model>(defaultModel());

  const [leastAlertingSource, setLeastAlertingSource] =
    createStore<Model>(defaultModel());

  const [mostAlertingDest, setMostAlertingDest] =
    createStore<Model>(defaultModel());

  const [leastAlertingDest, setLeastAlertingDest] =
    createStore<Model>(defaultModel());

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

            backgroundColor: dataValues.map(
              (_, index) => Colors[index % Colors.length],
            ),
            borderColor: dataValues.map(
              (_, index) => Colors[index % Colors.length],
            ),
          },
        ],
      },
      options: {
        plugins: {
          title: {
            display: true,
            text: "Alerts Over Time",
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
    console.log("Alerts.refresh");
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
      event_type: "alert",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      buildChart(response);
    });

    let loaders: {
      field: string;
      q: string;
      order: "desc" | "asc";
      setter: SetStoreFunction<Model>;
    }[] = [
      // Top alerting signatures.
      {
        field: "alert.signature",
        q: "event_type:alert",
        order: "desc",
        setter: setMostAlerting,
      },
      // Least alerting signatures.
      {
        field: "alert.signature",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastAlerting,
      },
      // Top alerting source addresses.
      {
        field: "src_ip",
        q: "event_type:alert",
        order: "desc",
        setter: setMostAlertingSource,
      },
      // Least alerting source addresses.
      {
        field: "src_ip",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastAlertingSource,
      },
      // Top alerting destination addresses.
      {
        field: "dest_ip",
        q: "event_type:alert",
        order: "desc",
        setter: setMostAlertingDest,
      },
      // Least alerting destination addresses.
      {
        field: "dest_ip",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastAlertingDest,
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

  return (
    <>
      <Top />

      <div class="container-fluid">
        <div class="row mt-2">
          <div class="col">
            <form class="row row-cols-lg-auto align-items-center">
              <div class="col-12">
                <RefreshButton loading={loading()} refresh={refresh} />
              </div>
              <div class="col-12">
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
                // blurInputs();
                // applyFilter(e.currentTarget.filter.value);
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
                  if (
                    e.code === "Escape" ||
                    e.key === "Escape" ||
                    e.keyCode === 27
                  ) {
                    // blurInputs();
                  }
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

        <div class="row">
          <div class="col mt-2">
            <canvas
              id="histogram"
              style="max-height: 250px; height: 300px"
            ></canvas>
          </div>
        </div>

        <div class="row mt-2">
          <div class="col">
            <CountValueDataTable
              title={"Most Alerting Signatures"}
              label={"Signature"}
              searchField="alert.signature"
              rows={mostAlerting.rows}
              loading={mostAlerting.loading}
              suffix={formatSuffix(mostAlerting.timestamp)}
            />
          </div>

          <div class="col">
            <CountValueDataTable
              title={"Least Alerting Signatures"}
              label={"Signature"}
              searchField="alert.signature"
              rows={leastAlerting.rows}
              loading={leastAlerting.loading}
              suffix={formatSuffix(leastAlerting.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Most Alerting Source Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={mostAlertingSource.rows}
              loading={mostAlertingSource.loading}
              suffix={formatSuffix(mostAlertingSource.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Alerting Source Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={leastAlertingSource.rows}
              loading={leastAlertingSource.loading}
              suffix={formatSuffix(leastAlertingSource.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Most Alerting Destination Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={mostAlertingDest.rows}
              loading={mostAlertingDest.loading}
              suffix={formatSuffix(mostAlertingDest.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Alerting Destination Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={leastAlertingDest.rows}
              loading={leastAlertingDest.loading}
              suffix={formatSuffix(leastAlertingDest.timestamp)}
            />
          </div>
        </div>
      </div>
    </>
  );
}
