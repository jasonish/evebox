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
  Model,
  percentOf,
  runAggLoaders,
  sumRows,
} from "./common";

interface Stats {
  sessions: number | null;
  ssh1: number | null;
  clients: number | null;
  servers: number | null;
  clientSoftware: number | null;
}

function defaultStats(): Stats {
  return {
    sessions: null,
    ssh1: null,
    clients: null,
    servers: null,
    clientSoftware: null,
  };
}

// SSH-1 protocol versions announced by servers: 1.x, including 1.99,
// which is SSH-2 with SSH-1 still enabled.
const isSsh1 = (version: string) => String(version).startsWith("1.");

export function SshDashboard() {
  const [version, setVersion] = createSignal(0);

  const [loading, setLoading] = createSignal(0);

  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
    q?: string;
  }>();

  const [mostClientSw, setMostClientSw] = createStore<Model>(defaultModel());
  const [mostServerSw, setMostServerSw] = createStore<Model>(defaultModel());
  const [leastClientSw, setLeastClientSw] = createStore<Model>(defaultModel());
  const [leastServerSw, setLeastServerSw] = createStore<Model>(defaultModel());
  const [clientHassh, setClientHassh] = createStore<Model>(defaultModel());
  const [serverHassh, setServerHassh] = createStore<Model>(defaultModel());
  const [topClients, setTopClients] = createStore<Model>(defaultModel());
  const [topServers, setTopServers] = createStore<Model>(defaultModel());
  const [otherPorts, setOtherPorts] = createStore<Model>(defaultModel());
  const [protoVersions, setProtoVersions] = createStore<Model>(defaultModel());
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
      event_type: "ssh",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      histogram = buildHistogram(
        "histogram",
        "SSH Events Over Time",
        response,
        histogram,
      );
    });

    const ssh = "event_type:ssh";
    const loaders: AggLoader[] = [
      {
        field: "ssh.client.software_version",
        q: ssh,
        order: "desc",
        setter: setMostClientSw,
      },
      {
        field: "ssh.server.software_version",
        q: ssh,
        order: "desc",
        setter: setMostServerSw,
      },
      {
        field: "ssh.client.software_version",
        q: ssh,
        order: "asc",
        setter: setLeastClientSw,
      },
      {
        field: "ssh.server.software_version",
        q: ssh,
        order: "asc",
        setter: setLeastServerSw,
      },
      {
        field: "ssh.client.hassh.hash",
        q: ssh,
        order: "desc",
        setter: setClientHassh,
      },
      {
        field: "ssh.server.hassh.hash",
        q: ssh,
        order: "desc",
        setter: setServerHassh,
      },
      { field: "src_ip", q: ssh, order: "desc", setter: setTopClients },
      { field: "dest_ip", q: ssh, order: "desc", setter: setTopServers },
      {
        field: "dest_port",
        q: `${ssh} -dest_port:22`,
        order: "desc",
        setter: setOtherPorts,
      },
      {
        field: "ssh.server.proto_version",
        q: ssh,
        order: "desc",
        setter: setProtoVersions,
      },
    ];
    runAggLoaders(loaders, queryString, timeRange, version, setLoading);

    setStats(defaultStats());
    loadingTracker(setLoading, async () => {
      const request = {
        size: 100,
        order: "desc" as const,
        time_range: timeRange,
        q: [...queryString, ssh].join(" "),
      };
      const distinct = { ...request, size: DISTINCT_LIMIT };
      const [sessions, versions, clients, servers, clientSoftware] =
        await Promise.all([
          fetchAgg({ ...request, field: "event_type" }),
          fetchAgg({ ...request, field: "ssh.server.proto_version" }),
          fetchAgg({ ...distinct, field: "src_ip" }),
          fetchAgg({ ...distinct, field: "dest_ip" }),
          fetchAgg({ ...distinct, field: "ssh.client.software_version" }),
        ]);
      setStats({
        sessions: sumRows(sessions.rows),
        ssh1: sumRows(versions.rows.filter((row) => isSsh1(row.key))),
        clients: clients.rows.length,
        servers: servers.rows.length,
        clientSoftware: clientSoftware.rows.length,
      });
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
              value={formatCount(stats.sessions)}
              label="SSH Sessions"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatCount(stats.ssh1)}
              label="SSH-1 Sessions"
              sub={percentOf(stats.ssh1, stats.sessions, "sessions")}
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.clients)}
              label="SSH Clients"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.servers)}
              label="SSH Servers"
            />
          </div>
          <div class="col-6 col-lg">
            <StatCard
              value={formatDistinct(stats.clientSoftware)}
              label="Client Software"
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
              title={"Most Common Client Software"}
              label={"Software"}
              searchField="ssh.client.software_version"
              rows={mostClientSw.rows}
              loading={mostClientSw.loading}
              suffix={formatSuffix(mostClientSw.timestamp)}
            />
          </div>
          <div class="col">
            <CountValueDataTable
              title={"Most Common Server Software"}
              label={"Software"}
              searchField="ssh.server.software_version"
              rows={mostServerSw.rows}
              loading={mostServerSw.loading}
              suffix={formatSuffix(mostServerSw.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Common Client Software"}
              label={"Software"}
              searchField="ssh.client.software_version"
              tooltip={
                "Rare client software can point to scanners, brute forcing tools or unusual automation."
              }
              rows={leastClientSw.rows}
              loading={leastClientSw.loading}
              suffix={formatSuffix(leastClientSw.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Least Common Server Software"}
              label={"Software"}
              searchField="ssh.server.software_version"
              rows={leastServerSw.rows}
              loading={leastServerSw.loading}
              suffix={formatSuffix(leastServerSw.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Client HASSH Fingerprints"}
              label={"HASSH"}
              searchField="ssh.client.hassh.hash"
              tooltip={
                "Fingerprints of the client's key exchange, identifying client software. Requires HASSH to be enabled in Suricata."
              }
              rows={clientHassh.rows}
              loading={clientHassh.loading}
              suffix={formatSuffix(clientHassh.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Server HASSH Fingerprints"}
              label={"HASSH"}
              searchField="ssh.server.hassh.hash"
              tooltip={
                "Fingerprints of the server's key exchange. Requires HASSH to be enabled in Suricata."
              }
              rows={serverHassh.rows}
              loading={serverHassh.loading}
              suffix={formatSuffix(serverHassh.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top SSH Clients"}
              label={"Address"}
              searchField="src_ip"
              rows={topClients.rows}
              loading={topClients.loading}
              suffix={formatSuffix(topClients.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Top SSH Servers"}
              label={"Address"}
              searchField="dest_ip"
              rows={topServers.rows}
              loading={topServers.loading}
              suffix={formatSuffix(topServers.timestamp)}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mt-2">
            <CountValueDataTable
              title={"SSH on Ports Other Than 22"}
              label={"Port"}
              searchField="dest_port"
              rows={otherPorts.rows}
              loading={otherPorts.loading}
              suffix={formatSuffix(otherPorts.timestamp)}
            />
          </div>
          <div class="col mt-2">
            <CountValueDataTable
              title={"Server Protocol Versions"}
              label={"Version"}
              searchField="ssh.server.proto_version"
              tooltip={
                "Protocol versions announced by servers. 1.x, including 1.99, means the insecure SSH-1 protocol is enabled."
              }
              rows={protoVersions.rows}
              loading={protoVersions.loading}
              suffix={formatSuffix(protoVersions.timestamp)}
            />
          </div>
        </div>
      </div>
    </>
  );
}
