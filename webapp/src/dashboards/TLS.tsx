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
  sessions: number | null;
  legacy: number | null;
  names: number | null;
  namesCapped: boolean;
  clients: number | null;
  clientsCapped: boolean;
  servers: number | null;
  serversCapped: boolean;
}

function defaultStats(): Stats {
  return {
    sessions: null,
    legacy: null,
    names: null,
    namesCapped: false,
    clients: null,
    clientsCapped: false,
    servers: null,
    serversCapped: false,
  };
}

// Protocol versions considered insecure, as logged by Suricata in
// tls.version. TLS 1.2, TLS 1.3 and TLS 1.3 drafts are not included.
const LEGACY_VERSIONS = ["SSLv2", "SSLv3", "TLSv1", "TLS 1.1"];

// Distinct counts are exact until this aggregation row limit is hit,
// then shown as a lower bound.
const DISTINCT_LIMIT = 500;

export function TlsDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [versions, setVersions] = createStore<Model>(defaultModel());

  const [legacyServers, setLegacyServers] = createStore<Model>(defaultModel());

  const [mostNames, setMostNames] = createStore<Model>(defaultModel());

  const [leastNames, setLeastNames] = createStore<Model>(defaultModel());

  const [issuers, setIssuers] = createStore<Model>(defaultModel());

  const [subjects, setSubjects] = createStore<Model>(defaultModel());

  const [ja4, setJa4] = createStore<Model>(defaultModel());

  const [ja3s, setJa3s] = createStore<Model>(defaultModel());

  const [otherPorts, setOtherPorts] = createStore<Model>(defaultModel());

  const [topServers, setTopServers] = createStore<Model>(defaultModel());

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
            text: "TLS Events Over Time",
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
      event_type: "tls",
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
      {
        field: "tls.version",
        q: "event_type:tls",
        order: "desc",
        setter: setVersions,
      },
      {
        field: "tls.sni",
        q: "event_type:tls",
        order: "desc",
        setter: setMostNames,
      },
      {
        field: "tls.sni",
        q: "event_type:tls",
        order: "asc",
        setter: setLeastNames,
      },
      {
        field: "tls.issuerdn",
        q: "event_type:tls",
        order: "desc",
        setter: setIssuers,
      },
      {
        field: "tls.subject",
        q: "event_type:tls",
        order: "desc",
        setter: setSubjects,
      },
      {
        field: "tls.ja4",
        q: "event_type:tls",
        order: "desc",
        setter: setJa4,
      },
      {
        field: "tls.ja3s.hash",
        q: "event_type:tls",
        order: "desc",
        setter: setJa3s,
      },
      {
        field: "dest_port",
        q: "event_type:tls -dest_port:443",
        order: "desc",
        setter: setOtherPorts,
      },
      {
        field: "dest_ip",
        q: "event_type:tls",
        order: "desc",
        setter: setTopServers,
      },
    ];

    // Stat cards, and the servers still negotiating legacy versions,
    // merged from one aggregation per legacy version seen.
    setStats(defaultStats());
    setLegacyServers("loading", true);
    loadingTracker(setLoading, async () => {
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
      };
      const tls = [...queryString, "event_type:tls"].join(" ");
      const [versionRows, names, clients, servers] = await Promise.all([
        fetchAgg({ ...request, field: "tls.version", q: tls }),
        fetchAgg({
          ...request,
          size: DISTINCT_LIMIT,
          field: "tls.sni",
          q: tls,
        }),
        fetchAgg({
          ...request,
          size: DISTINCT_LIMIT,
          field: "src_ip",
          q: tls,
        }),
        fetchAgg({
          ...request,
          size: DISTINCT_LIMIT,
          field: "dest_ip",
          q: tls,
        }),
      ]);
      const legacy = versionRows.rows.filter((row) =>
        LEGACY_VERSIONS.includes(row.key),
      );
      setStats({
        sessions: versionRows.rows.reduce((acc, row) => acc + row.count, 0),
        legacy: legacy.reduce((acc, row) => acc + row.count, 0),
        names: names.rows.length,
        namesCapped: names.rows.length >= DISTINCT_LIMIT,
        clients: clients.rows.length,
        clientsCapped: clients.rows.length >= DISTINCT_LIMIT,
        servers: servers.rows.length,
        serversCapped: servers.rows.length >= DISTINCT_LIMIT,
      });

      const perVersion = await Promise.all(
        legacy.map((row) =>
          fetchAgg({
            ...request,
            field: "dest_ip",
            q: [...queryString, `event_type:tls tls.version:"${row.key}"`].join(
              " ",
            ),
          }),
        ),
      );
      const merged: Map<string, number> = new Map();
      for (const response of perVersion) {
        for (const row of response.rows) {
          merged.set(row.key, (merged.get(row.key) || 0) + row.count);
        }
      }
      const rows = [...merged.entries()]
        .map(([key, count]) => ({ key, count }))
        .sort((a, b) => b.count - a.count)
        .slice(0, 10);
      setLegacyServers("rows", rows);
    }).finally(() => {
      setLegacyServers("loading", false);
    });

    for (const loader of loaders) {
      loader.setter("loading", true);

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
        loader.setter("loading", false);
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

  const formatDistinct = (count: number | null, capped: boolean) =>
    capped ? `${count}+` : formatCount(count);

  const percentOfSessions = (count: number | null) => {
    if (count === null || !stats.sessions) {
      return undefined;
    }
    const percent = (count / stats.sessions) * 100;
    if (count > 0 && percent < 0.1) {
      return "<0.1% of TLS events";
    }
    return `${percent.toFixed(1)}% of TLS events`;
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
            <StatCard value={formatCount(stats.sessions)} label="TLS Events" />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.legacy)}
              label="Legacy TLS/SSL"
              sub={percentOfSessions(stats.legacy)}
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.names, stats.namesCapped)}
              label="Server Names"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.clients, stats.clientsCapped)}
              label="TLS Clients"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.servers, stats.serversCapped)}
              label="TLS Servers"
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
              title={"TLS Versions"}
              label={"Version"}
              searchField="tls.version"
              tooltip={
                "Negotiated protocol versions. SSLv2, SSLv3, TLS 1.0 (TLSv1) and TLS 1.1 are deprecated and insecure."
              }
              rows={versions.rows}
              loading={versions.loading}
              suffix={formatSuffix(versions.timestamp)}
            />
          </div>
          <div class="col">
            <CountValueDataTable
              title={"Legacy TLS/SSL Servers"}
              label={"Address"}
              searchField="dest_ip"
              tooltip={"Servers negotiating SSLv2, SSLv3, TLS 1.0 or TLS 1.1."}
              rows={legacyServers.rows}
              loading={legacyServers.loading}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Most Requested Server Names"}
              label={"Server Name (SNI)"}
              searchField="tls.sni"
              rows={mostNames.rows}
              loading={mostNames.loading}
              suffix={formatSuffix(mostNames.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Requested Server Names"}
              label={"Server Name (SNI)"}
              searchField="tls.sni"
              rows={leastNames.rows}
              loading={leastNames.loading}
              suffix={formatSuffix(leastNames.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top Certificate Issuers"}
              label={"Issuer"}
              searchField="tls.issuerdn"
              tooltip={
                "Only logged when the certificate is seen. TLS 1.3 encrypts the certificate, so these come from older versions."
              }
              rows={issuers.rows}
              loading={issuers.loading}
              suffix={formatSuffix(issuers.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top Certificate Subjects"}
              label={"Subject"}
              searchField="tls.subject"
              tooltip={
                "Only logged when the certificate is seen. TLS 1.3 encrypts the certificate, so these come from older versions."
              }
              rows={subjects.rows}
              loading={subjects.loading}
              suffix={formatSuffix(subjects.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top JA4 Client Fingerprints"}
              label={"JA4"}
              searchField="tls.ja4"
              tooltip={
                "Fingerprints of the TLS client hello, identifying client software. Requires JA4 to be enabled in Suricata."
              }
              rows={ja4.rows}
              loading={ja4.loading}
              suffix={formatSuffix(ja4.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top JA3S Server Fingerprints"}
              label={"JA3S"}
              searchField="tls.ja3s.hash"
              tooltip={
                "Fingerprints of the TLS server hello. Requires JA3 to be enabled in Suricata."
              }
              rows={ja3s.rows}
              loading={ja3s.loading}
              suffix={formatSuffix(ja3s.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"TLS on Ports Other Than 443"}
              label={"Port"}
              searchField="dest_port"
              rows={otherPorts.rows}
              loading={otherPorts.loading}
              suffix={formatSuffix(otherPorts.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top TLS Servers"}
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
