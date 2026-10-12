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
  events: number | null;
  legacy: number | null;
  names: number | null;
  clients: number | null;
  servers: number | null;
}

function defaultStats(): Stats {
  return {
    events: null,
    legacy: null,
    names: null,
    clients: null,
    servers: null,
  };
}

// Protocol versions considered insecure, as logged by Suricata in
// tls.version. TLS 1.2, TLS 1.3 and TLS 1.3 drafts are not included.
const LEGACY_VERSIONS = ["SSLv2", "SSLv3", "TLSv1", "TLS 1.1"];

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
      event_type: "tls",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      histogram = buildHistogram(
        "histogram",
        "TLS Events Over Time",
        response,
        histogram,
      );
    });

    const tls = "event_type:tls";
    const loaders: AggLoader[] = [
      { field: "tls.version", q: tls, order: "desc", setter: setVersions },
      { field: "tls.sni", q: tls, order: "desc", setter: setMostNames },
      { field: "tls.sni", q: tls, order: "asc", setter: setLeastNames },
      { field: "tls.issuerdn", q: tls, order: "desc", setter: setIssuers },
      { field: "tls.subject", q: tls, order: "desc", setter: setSubjects },
      { field: "tls.ja4", q: tls, order: "desc", setter: setJa4 },
      { field: "tls.ja3s.hash", q: tls, order: "desc", setter: setJa3s },
      {
        field: "dest_port",
        q: `${tls} -dest_port:443`,
        order: "desc",
        setter: setOtherPorts,
      },
      { field: "dest_ip", q: tls, order: "desc", setter: setTopServers },
    ];
    runAggLoaders(loaders, queryString, timeRange, version, setLoading);

    // Stat cards, and the servers still negotiating legacy versions,
    // merged from one aggregation per legacy version seen.
    setStats(defaultStats());
    setLegacyServers("loading", true);
    loadingTracker(setLoading, async () => {
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
        q: [...queryString, tls].join(" "),
      };
      const distinct = { ...request, size: DISTINCT_LIMIT };
      const [events, versionRows, names, clients, servers] = await Promise.all([
        fetchAgg({ ...request, field: "event_type" }),
        fetchAgg({ ...request, field: "tls.version" }),
        fetchAgg({ ...distinct, field: "tls.sni" }),
        fetchAgg({ ...distinct, field: "src_ip" }),
        fetchAgg({ ...distinct, field: "dest_ip" }),
      ]);
      const legacy = versionRows.rows.filter((row) =>
        LEGACY_VERSIONS.includes(row.key),
      );
      setStats({
        events: sumRows(events.rows),
        legacy: sumRows(legacy),
        names: names.rows.length,
        clients: clients.rows.length,
        servers: servers.rows.length,
      });

      const perVersion = await Promise.all(
        legacy.map((row) =>
          fetchAgg({
            ...request,
            field: "dest_ip",
            q: `${request.q} tls.version:"${row.key}"`,
          }),
        ),
      );
      setLegacyServers("rows", mergeRows(perVersion));
    }).finally(() => {
      setLegacyServers("loading", false);
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
            <StatCard value={formatCount(stats.events)} label="TLS Events" />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.legacy)}
              label="Legacy TLS/SSL"
              sub={percentOf(stats.legacy, stats.events, "TLS events")}
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.names)}
              label="Server Names"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.clients)}
              label="TLS Clients"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.servers)}
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
