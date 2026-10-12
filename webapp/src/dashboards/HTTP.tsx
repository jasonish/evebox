// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal, onCleanup } from "solid-js";
import { API, fetchAgg } from "../api";
import { Chart } from "chart.js";
import { useSearchParams } from "@solidjs/router";
import { loadingTracker } from "../util";
import { CountValueDataTable } from "../components/CountValueDataTable";
import { StatCard } from "../components/StatCard";
import { createStore } from "solid-js/store";
import {
  AggLoader,
  DISTINCT_LIMIT,
  DashboardFilters,
  buildHistogram,
  defaultModel,
  formatCount,
  formatDistinct,
  formatSuffix,
  mergeRows,
  Model,
  percentOf,
  runAggLoaders,
  sumRows,
} from "./common";

interface Stats {
  requests: number | null;
  // Null when no status codes were logged.
  errors: number | null;
  hosts: number | null;
  clients: number | null;
  servers: number | null;
}

function defaultStats(): Stats {
  return {
    requests: null,
    errors: null,
    hosts: null,
    clients: null,
    servers: null,
  };
}

// Content types of executable downloads, compared without parameters.
const EXECUTABLE_TYPES = [
  "application/x-dosexec",
  "application/x-msdownload",
  "application/x-msdos-program",
  "application/vnd.microsoft.portable-executable",
  "application/x-msi",
  "application/x-executable",
  "application/x-elf",
  "application/x-mach-binary",
  "application/vnd.android.package-archive",
];

const isExecutableType = (contentType: string) =>
  EXECUTABLE_TYPES.includes(contentType.split(";")[0].trim().toLowerCase());

const EXTENDED_TOOLTIP =
  "Requires extended HTTP logging (extended: yes for the http logger in suricata.yaml).";

export function HttpDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [mostHosts, setMostHosts] = createStore<Model>(defaultModel());
  const [leastHosts, setLeastHosts] = createStore<Model>(defaultModel());
  const [urls, setUrls] = createStore<Model>(defaultModel());
  const [statuses, setStatuses] = createStore<Model>(defaultModel());
  const [mostAgents, setMostAgents] = createStore<Model>(defaultModel());
  const [leastAgents, setLeastAgents] = createStore<Model>(defaultModel());
  const [methods, setMethods] = createStore<Model>(defaultModel());
  const [contentTypes, setContentTypes] = createStore<Model>(defaultModel());
  const [exeHosts, setExeHosts] = createStore<Model>(defaultModel());
  const [otherPorts, setOtherPorts] = createStore<Model>(defaultModel());
  const [referrers, setReferrers] = createStore<Model>(defaultModel());
  const [topServers, setTopServers] = createStore<Model>(defaultModel());
  const [stats, setStats] = createStore<Stats>(defaultStats());

  let histogram: Chart | undefined = undefined;

  onCleanup(() => {
    API.cancelAllSse();
  });

  createEffect(() => {
    refresh();
  });

  function refresh() {
    setVersion((version) => version + 1);
    const timeRange = TIME_RANGE();

    const queryString: string[] = [];
    if (searchParams.q) {
      queryString.push(searchParams.q);
    }
    if (searchParams.sensor) {
      queryString.push(`host:${searchParams.sensor}`);
    }

    API.histogramTime({
      time_range: timeRange,
      event_type: "http",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      histogram = buildHistogram(
        "histogram",
        "HTTP Events Over Time",
        response,
        histogram,
      );
    });

    const http = "event_type:http";
    const loaders: AggLoader[] = [
      { field: "http.hostname", q: http, order: "desc", setter: setMostHosts },
      { field: "http.hostname", q: http, order: "asc", setter: setLeastHosts },
      { field: "http.url", q: http, order: "desc", setter: setUrls },
      { field: "http.status", q: http, order: "desc", setter: setStatuses },
      {
        field: "http.http_user_agent",
        q: http,
        order: "desc",
        setter: setMostAgents,
      },
      {
        field: "http.http_user_agent",
        q: http,
        order: "asc",
        setter: setLeastAgents,
      },
      { field: "http.http_method", q: http, order: "desc", setter: setMethods },
      {
        field: "http.http_content_type",
        q: http,
        order: "desc",
        setter: setContentTypes,
      },
      {
        field: "dest_port",
        q: `${http} -dest_port:80`,
        order: "desc",
        setter: setOtherPorts,
      },
      {
        field: "http.http_refer",
        q: http,
        order: "desc",
        setter: setReferrers,
      },
      { field: "dest_ip", q: http, order: "desc", setter: setTopServers },
    ];
    runAggLoaders(loaders, queryString, timeRange, version, setLoading);

    // Stat cards, and the hosts serving executables, merged from one
    // aggregation per executable content type seen.
    setStats(defaultStats());
    setExeHosts("loading", true);
    loadingTracker(setLoading, async () => {
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
        q: [...queryString, http].join(" "),
      };
      const distinct = { ...request, size: DISTINCT_LIMIT };
      const [requests, statusRows, types, hosts, clients, servers] =
        await Promise.all([
          fetchAgg({ ...request, field: "event_type" }),
          fetchAgg({ ...request, field: "http.status" }),
          fetchAgg({ ...request, size: 200, field: "http.http_content_type" }),
          fetchAgg({ ...distinct, field: "http.hostname" }),
          fetchAgg({ ...distinct, field: "src_ip" }),
          fetchAgg({ ...distinct, field: "dest_ip" }),
        ]);
      setStats({
        requests: sumRows(requests.rows),
        errors:
          statusRows.rows.length > 0
            ? sumRows(statusRows.rows.filter((row) => Number(row.key) >= 400))
            : null,
        hosts: hosts.rows.length,
        clients: clients.rows.length,
        servers: servers.rows.length,
      });

      const perType = await Promise.all(
        types.rows
          .filter((row) => isExecutableType(String(row.key)))
          .map((row) =>
            fetchAgg({
              ...request,
              field: "http.hostname",
              q: `${request.q} http.http_content_type:"${row.key}"`,
            }),
          ),
      );
      setExeHosts("rows", mergeRows(perType));
    }).finally(() => {
      setExeHosts("loading", false);
    });
  }

  return (
    <>
      <Top />

      <div class="container-fluid">
        <DashboardFilters
          loading={loading()}
          refresh={refresh}
          sensor={searchParams.sensor}
          setSensor={(sensor) => setSearchParams({ sensor: sensor })}
          q={searchParams.q}
          setQuery={(q) => setSearchParams({ q: q })}
        />

        <div class="row mt-2 g-2">
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.requests)}
              label="HTTP Requests"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.errors)}
              label="Error Responses"
              sub={
                stats.requests && stats.errors === null
                  ? "needs extended logging"
                  : percentOf(stats.errors, stats.requests, "requests")
              }
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard value={formatDistinct(stats.hosts)} label="Hosts" />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.clients)}
              label="HTTP Clients"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.servers)}
              label="HTTP Servers"
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
              title={"Most Requested Hosts"}
              label={"Host"}
              searchField="http.hostname"
              rows={mostHosts.rows}
              loading={mostHosts.loading}
              suffix={formatSuffix(mostHosts.timestamp)}
            />
          </div>
          <div class="col">
            <CountValueDataTable
              title={"Least Requested Hosts"}
              label={"Host"}
              searchField="http.hostname"
              rows={leastHosts.rows}
              loading={leastHosts.loading}
              suffix={formatSuffix(leastHosts.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top URLs"}
              label={"URL"}
              searchField="http.url"
              rows={urls.rows}
              loading={urls.loading}
              suffix={formatSuffix(urls.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Content Types"}
              label={"Content Type"}
              searchField="http.http_content_type"
              rows={contentTypes.rows}
              loading={contentTypes.loading}
              suffix={formatSuffix(contentTypes.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Most Common User Agents"}
              label={"User Agent"}
              searchField="http.http_user_agent"
              rows={mostAgents.rows}
              loading={mostAgents.loading}
              suffix={formatSuffix(mostAgents.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Common User Agents"}
              label={"User Agent"}
              searchField="http.http_user_agent"
              tooltip={
                "Rare user agents can point to unusual software or scripted clients."
              }
              rows={leastAgents.rows}
              loading={leastAgents.loading}
              suffix={formatSuffix(leastAgents.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Request Methods"}
              label={"Method"}
              searchField="http.http_method"
              tooltip={EXTENDED_TOOLTIP}
              rows={methods.rows}
              loading={methods.loading}
              suffix={formatSuffix(methods.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Status Codes"}
              label={"Status"}
              searchField="http.status"
              tooltip={EXTENDED_TOOLTIP}
              rows={statuses.rows}
              loading={statuses.loading}
              suffix={formatSuffix(statuses.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Hosts Serving Executables"}
              label={"Host"}
              searchField="http.hostname"
              tooltip={
                "Hosts responding with an executable content type, such as Windows PE, ELF, Mach-O, MSI or Android packages."
              }
              rows={exeHosts.rows}
              loading={exeHosts.loading}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"HTTP on Ports Other Than 80"}
              label={"Port"}
              searchField="dest_port"
              rows={otherPorts.rows}
              loading={otherPorts.loading}
              suffix={formatSuffix(otherPorts.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top Referrers"}
              label={"Referrer"}
              searchField="http.http_refer"
              tooltip={EXTENDED_TOOLTIP}
              rows={referrers.rows}
              loading={referrers.loading}
              suffix={formatSuffix(referrers.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top HTTP Servers"}
              label={"Address"}
              searchField="dest_ip"
              rows={topServers.rows}
              loading={topServers.loading}
              suffix={formatSuffix(topServers.timestamp)}
            />
          </div>
        </div>
      </div>
    </>
  );
}
