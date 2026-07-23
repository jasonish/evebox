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
  queries: number | null;
  responses: number | null;
  nxdomain: number | null;
  servfail: number | null;
  clients: number | null;
  clientsCapped: boolean;
  servers: number | null;
  serversCapped: boolean;
}

function defaultStats(): Stats {
  return {
    queries: null,
    responses: null,
    nxdomain: null,
    servfail: null,
    clients: null,
    clientsCapped: false,
    servers: null,
    serversCapped: false,
  };
}

// Distinct client and server counts are exact until these aggregation
// row limits are hit, then shown as a lower bound.
const CLIENT_LIMIT = 500;
const SERVER_LIMIT = 100;

export function DnsDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [mostRequested, setMostRequested] = createStore<Model>(defaultModel());

  const [leastRequested, setLeastRequested] =
    createStore<Model>(defaultModel());

  const [topClients, setTopClients] = createStore<Model>(defaultModel());

  const [topServers, setTopServers] = createStore<Model>(defaultModel());

  const [nxdomainNames, setNxdomainNames] = createStore<Model>(defaultModel());

  const [nxdomainClients, setNxdomainClients] =
    createStore<Model>(defaultModel());

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
            text: "DNS Events Over Time",
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
      event_type: "dns",
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
      // Most requested DNS names.
      {
        field: "dns.rrname",
        q: "event_type:dns dns.type:query",
        order: "desc",
        setter: setMostRequested,
      },
      // Least requested DNS names.
      {
        field: "dns.rrname",
        q: "event_type:dns dns.type:query",
        order: "asc",
        setter: setLeastRequested,
      },
      // Top DNS clients, the source of query events.
      {
        field: "src_ip",
        q: "event_type:dns dns.type:query",
        order: "desc",
        setter: setTopClients,
      },
      // Top DNS servers, the destination of query events.
      {
        field: "dest_ip",
        q: "event_type:dns dns.type:query",
        order: "desc",
        setter: setTopServers,
      },
      // Top NXDOMAIN names, from response events.
      {
        field: "dns.rrname",
        q: "event_type:dns dns.type:answer dns.rcode:NXDOMAIN",
        order: "desc",
        setter: setNxdomainNames,
      },
    ];

    // Stat cards. Message-type and response-code aggregations provide
    // the counts, and the distinct servers seen come from a dest_ip
    // aggregation over query events, exact as long as the row limit is
    // not hit.
    setStats(defaultStats());
    loadingTracker(setLoading, async () => {
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
      };
      const [types, rcodes, clients, servers] = await Promise.all([
        fetchAgg({
          ...request,
          field: "dns.type",
          q: [...queryString, "event_type:dns"].join(" "),
        }),
        fetchAgg({
          ...request,
          field: "dns.rcode",
          q: [...queryString, "event_type:dns dns.type:answer"].join(" "),
        }),
        fetchAgg({
          ...request,
          size: CLIENT_LIMIT,
          field: "src_ip",
          q: [...queryString, "event_type:dns dns.type:query"].join(" "),
        }),
        fetchAgg({
          ...request,
          size: SERVER_LIMIT,
          field: "dest_ip",
          q: [...queryString, "event_type:dns dns.type:query"].join(" "),
        }),
      ]);
      const typeCount = (names: string[]) =>
        types.rows
          .filter((row) => names.includes(row.key))
          .reduce((acc, row) => acc + row.count, 0);
      const rcodeCount = (name: string) =>
        rcodes.rows.find((row) => row.key === name)?.count || 0;
      setStats({
        queries: typeCount(["query", "request"]),
        responses: typeCount(["answer", "response"]),
        nxdomain: rcodeCount("NXDOMAIN"),
        servfail: rcodeCount("SERVFAIL"),
        clients: clients.rows.length,
        clientsCapped: clients.rows.length >= CLIENT_LIMIT,
        servers: servers.rows.length,
        serversCapped: servers.rows.length >= SERVER_LIMIT,
      });
    });

    // Top NXDOMAIN clients. EVE DNS v2 logs responses with the flow
    // tuple, the client as the source, while v3 logs the packet tuple,
    // the server as the source. The server side of a response sits on
    // port 53, so take the client from the other side and merge.
    setNxdomainClients("loading", true);
    loadingTracker(setLoading, async () => {
      const nxdomain = "event_type:dns dns.type:answer dns.rcode:NXDOMAIN";
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
      };
      const [flowTuple, packetTuple] = await Promise.all([
        fetchAgg({
          ...request,
          field: "src_ip",
          q: [...queryString, nxdomain, "-src_port:53"].join(" "),
        }),
        fetchAgg({
          ...request,
          field: "dest_ip",
          q: [...queryString, nxdomain, "src_port:53"].join(" "),
        }),
      ]);
      const merged: Map<string, number> = new Map();
      for (const row of [...flowTuple.rows, ...packetTuple.rows]) {
        merged.set(row.key, (merged.get(row.key) || 0) + row.count);
      }
      const rows = [...merged.entries()]
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count)
        .slice(0, 10);
      setNxdomainClients("rows", rows);
    }).finally(() => {
      setNxdomainClients("loading", false);
    });

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

  const percentOfResponses = (count: number | null) => {
    if (count === null || !stats.responses) {
      return undefined;
    }
    return `${((count / stats.responses) * 100).toFixed(1)}% of responses`;
  };

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
            <StatCard value={formatCount(stats.queries)} label="DNS Queries" />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.nxdomain)}
              label="NXDOMAIN"
              sub={percentOfResponses(stats.nxdomain)}
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.servfail)}
              label="SERVFAIL"
              sub={percentOfResponses(stats.servfail)}
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={
                stats.clientsCapped
                  ? `${stats.clients}+`
                  : formatCount(stats.clients)
              }
              label="DNS Clients"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={
                stats.serversCapped
                  ? `${stats.servers}+`
                  : formatCount(stats.servers)
              }
              label="DNS Servers"
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
              title={"Most Requested DNS Names"}
              label={"Name"}
              searchField="dns.rrname"
              rows={mostRequested.rows}
              loading={mostRequested.loading}
              suffix={formatSuffix(mostRequested.timestamp)}
            />
          </div>

          <div class="col">
            <CountValueDataTable
              title={"Least Requested DNS Names"}
              label={"Name"}
              searchField="dns.rrname"
              rows={leastRequested.rows}
              loading={leastRequested.loading}
              suffix={formatSuffix(leastRequested.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top DNS Clients"}
              label={"Address"}
              searchField={"@ip"}
              tooltip={"Source addresses of DNS query events."}
              rows={topClients.rows}
              loading={topClients.loading}
              suffix={formatSuffix(topClients.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top DNS Servers"}
              label={"Address"}
              searchField={"@ip"}
              tooltip={"Destination addresses of DNS query events."}
              rows={topServers.rows}
              loading={topServers.loading}
              suffix={formatSuffix(topServers.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top NXDOMAIN Names"}
              label={"Name"}
              searchField="dns.rrname"
              tooltip={
                "Names that failed to resolve. Spikes can indicate misconfigurations, typos, or malware probing for command and control domains."
              }
              rows={nxdomainNames.rows}
              loading={nxdomainNames.loading}
              suffix={formatSuffix(nxdomainNames.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top NXDOMAIN Clients"}
              label={"Address"}
              searchField={"@ip"}
              tooltip={
                "Clients receiving NXDOMAIN responses, the recipients of failed name lookups."
              }
              rows={nxdomainClients.rows}
              loading={nxdomainClients.loading}
              suffix={formatSuffix(nxdomainClients.timestamp)}
            />
          </div>
        </div>
      </div>
    </>
  );
}
