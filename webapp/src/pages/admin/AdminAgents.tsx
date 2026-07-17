// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  For,
  Show,
  createMemo,
  createResource,
  createSignal,
  onCleanup,
} from "solid-js";
import { Top } from "../../Top";
import { API } from "../../api";
import { parse_timestamp } from "../../datetime";

// An agent is stale when nothing has been seen from it for this many
// seconds. The server pings agents every 30 seconds, dropping them
// after 2 missed pings, so 90 seconds means something is wrong.
const STALE_SECONDS = 90;

// One display row: an agent — connected, key-only, or both, merged by
// name since a key's name is the agent identity it authorizes — or the
// server-local pcap source.
interface AgentRow {
  name: string;
  kind: "agent" | "server";
  hostname?: string;
  version?: string;
  capabilities: string[];
  connected: boolean;
  // The live connection's last-seen for connected agents, the key's
  // stored last-seen for offline ones.
  last_seen?: string;
  rtt_ms?: number;
  key?: API.AgentKeyInfo;
}

// The server-local row comes from GET /api/pcap/sources. A 404 means
// this build has no pcap routes (e.g. Windows) and so no server-local
// source; any other failure propagates so the poll keeps the last
// good data and shows the warning banner instead of rendering a false
// "no agents" empty state.
async function fetchServerSources(): Promise<API.PcapSource[]> {
  try {
    return await API.getPcapSources();
  } catch (e: any) {
    if (e?.status === 404) {
      return [];
    }
    throw e;
  }
}

// Merge connected agents (GET /api/agents) with issued agent keys
// (GET /api/agents/keys) by name, then append the server-local pcap
// source when one is configured.
async function fetchAgents(): Promise<AgentRow[]> {
  const [agents, keys, pcapSources] = await Promise.all([
    API.getAgents(),
    API.getAgentKeys(),
    fetchServerSources(),
  ]);
  const byName = new Map<string, AgentRow>();
  for (const agent of agents) {
    byName.set(agent.name, {
      name: agent.name,
      kind: "agent",
      hostname: agent.hostname,
      version: agent.version,
      capabilities: agent.capabilities,
      connected: true,
      last_seen: agent.last_seen,
      rtt_ms: agent.rtt_ms,
    });
  }
  for (const key of keys) {
    const row = byName.get(key.name);
    if (row) {
      row.key = key;
    } else {
      byName.set(key.name, {
        name: key.name,
        kind: "agent",
        capabilities: [],
        connected: false,
        last_seen: key.last_seen ?? undefined,
        key: key,
      });
    }
  }
  const rows = Array.from(byName.values());
  for (const source of pcapSources) {
    if (source.kind === "server") {
      rows.push({
        name: source.name,
        kind: "server",
        capabilities: ["pcap"],
        connected: true,
      });
    }
  }
  return rows;
}

// The server-local source is always live; agents are live if seen
// within the stale window, key-only rows are offline.
function rowStatus(row: AgentRow): "live" | "stale" | "offline" {
  if (row.kind === "server") {
    return "live";
  }
  if (!row.connected || !row.last_seen) {
    return "offline";
  }
  const lastSeen = parse_timestamp(row.last_seen);
  return Date.now() - lastSeen.valueOf() < STALE_SECONDS * 1000
    ? "live"
    : "stale";
}

type SortColumn = "name" | "hostname" | "version" | "last_seen";

// Natural-order compare so version "0.27.10" sorts after "0.27.2" and
// hostname "sensor-10" after "sensor-9".
function compareText(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true });
}

// Rows without a value for the sort column go last whatever the
// direction; ties fall back to name so the order is stable.
function compareRows(
  a: AgentRow,
  b: AgentRow,
  column: SortColumn,
  asc: boolean,
): number {
  const direction = asc ? 1 : -1;
  if (column === "last_seen") {
    const av = a.last_seen ? parse_timestamp(a.last_seen).valueOf() : undefined;
    const bv = b.last_seen ? parse_timestamp(b.last_seen).valueOf() : undefined;
    if (av === undefined && bv === undefined)
      return compareText(a.name, b.name);
    if (av === undefined) return 1;
    if (bv === undefined) return -1;
    return (av - bv) * direction || compareText(a.name, b.name);
  }
  const av = column === "name" ? a.name : a[column];
  const bv = column === "name" ? b.name : b[column];
  if (av === undefined && bv === undefined) return compareText(a.name, b.name);
  if (av === undefined) return 1;
  if (bv === undefined) return -1;
  return compareText(av, bv) * direction || compareText(a.name, b.name);
}

// Copy `text` to the clipboard. navigator.clipboard only exists in
// secure contexts and EveBox is commonly served over plain HTTP, so
// fall back to a hidden textarea + execCommand there.
async function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard) {
    return navigator.clipboard.writeText(text);
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!ok) {
    throw new Error("copy failed");
  }
}

function CopyButton(props: { text: string }) {
  const [state, setState] = createSignal<"idle" | "copied" | "failed">("idle");
  const flash = (result: "copied" | "failed") => {
    setState(result);
    setTimeout(() => setState("idle"), 2000);
  };
  return (
    <button
      type="button"
      class={
        "btn btn-sm " +
        (state() === "copied"
          ? "btn-success"
          : state() === "failed"
            ? "btn-danger"
            : "btn-outline-secondary")
      }
      onClick={() => {
        copyToClipboard(props.text)
          .then(() => flash("copied"))
          .catch(() => flash("failed"));
      }}
    >
      {state() === "copied"
        ? "Copied"
        : state() === "failed"
          ? "Copy failed"
          : "Copy"}
    </button>
  );
}

export function AdminAgents() {
  const [fetchError, setFetchError] = createSignal(false);

  // Catch fetch failures so a failed poll doesn't put the resource in
  // an error state, which would throw on reads of agents.latest.
  // Instead keep the last good data and note the error above the table.
  const [agents, { refetch }] = createResource<AgentRow[] | undefined>(
    async (_k, info) => {
      try {
        const rows = await fetchAgents();
        setFetchError(false);
        return rows;
      } catch (_e) {
        setFetchError(true);
        return info.value;
      }
    },
  );

  const timer = setInterval(() => {
    refetch();
  }, 5000);
  onCleanup(() => clearInterval(timer));

  const [sortColumn, setSortColumn] = createSignal<SortColumn>("name");
  const [sortAsc, setSortAsc] = createSignal(true);

  const sorted = createMemo(() => {
    const rows = [...(agents.latest ?? [])];
    rows.sort((a, b) => compareRows(a, b, sortColumn(), sortAsc()));
    return rows;
  });

  const setSort = (column: SortColumn) => {
    if (sortColumn() === column) {
      setSortAsc(!sortAsc());
    } else {
      setSortColumn(column);
      setSortAsc(true);
    }
  };

  const sortIndicator = (column: SortColumn) =>
    sortColumn() === column ? (sortAsc() ? " ▲" : " ▼") : "";

  // Key management state.
  const [name, setName] = createSignal("");
  const [added, setAdded] = createSignal<API.AgentKeyWithSecret | null>(null);
  const [actionError, setActionError] = createSignal<string | null>(null);
  // Keys revealed into the table, by key id. Keys are re-showable on
  // demand but never rendered by default.
  const [revealed, setRevealed] = createSignal<{ [id: number]: string }>({});

  const hideKey = (id: number) => {
    const next = { ...revealed() };
    delete next[id];
    setRevealed(next);
  };

  // Guard against double-submits: a second create for the same name
  // would fail on the unique-name constraint and its error handling
  // would tear down the panel showing the just-created key.
  const [creating, setCreating] = createSignal(false);

  const createKey = async (keyName: string) => {
    if (creating()) {
      return;
    }
    setCreating(true);
    try {
      const created = await API.addAgentKey(keyName);
      setName("");
      setActionError(null);
      setAdded(created);
    } catch (e: any) {
      setAdded(null);
      setActionError(e.message);
    }
    setCreating(false);
    refetch();
  };

  const submitCreate = (e: Event) => {
    e.preventDefault();
    createKey(name());
  };

  const toggleReveal = async (key: API.AgentKeyInfo) => {
    if (revealed()[key.id] !== undefined) {
      hideKey(key.id);
      return;
    }
    try {
      const row = await API.revealAgentKey(key.id);
      setRevealed({ ...revealed(), [key.id]: row.key });
      setActionError(null);
    } catch (e: any) {
      setActionError(e.message);
      refetch();
    }
  };

  const deleteKey = async (key: API.AgentKeyInfo) => {
    if (!confirm(`Delete the agent key for "${key.name}"?`)) {
      return;
    }
    try {
      await API.deleteAgentKey(key.id);
      setActionError(null);
      if (added()?.id === key.id) {
        setAdded(null);
      }
      hideKey(key.id);
    } catch (e: any) {
      setActionError(e.message);
    }
    refetch();
  };

  return (
    <>
      <Top />
      <div class="container mt-2">
        <div class="row">
          <div class="col">
            <h2>Agents</h2>
          </div>
        </div>

        <Show when={fetchError()}>
          <div class="alert alert-warning">
            Failed to load agents, will keep trying.
          </div>
        </Show>

        <Show when={actionError()}>
          <div class="alert alert-danger">{actionError()}</div>
        </Show>

        <div class="card">
          <form class="card-body" onSubmit={submitCreate}>
            <div class="input-group">
              <input
                type="text"
                class="form-control"
                placeholder="Agent ID (name)"
                value={name()}
                onInput={(e) => setName(e.currentTarget.value)}
              />
              <button
                class="btn btn-primary"
                type="submit"
                disabled={creating() || name().trim().length === 0}
              >
                Add Agent
              </button>
            </div>
          </form>
        </div>

        <Show when={added()}>
          <div class="alert alert-success mt-2 mb-0">
            <div>
              Agent key added for <b>{added()!.name}</b>:
            </div>
            <div class="d-flex align-items-center mt-1">
              <code class="me-2 text-break user-select-all">
                {added()!.key}
              </code>
              <CopyButton text={added()!.key} />
            </div>
            <div class="mt-1">
              Set this as <code>server.key</code> in the agent's{" "}
              <code>agent.yaml</code> (or <code>EVEBOX_SERVER_KEY</code>).
            </div>
          </div>
        </Show>

        <Show when={agents.latest}>
          <Show
            when={sorted().length > 0}
            fallback={
              <div class="card mt-2">
                <div class="card-body">
                  No agents yet. Connected agents, issued agent keys, and the
                  server-local pcap spool will appear here.
                </div>
              </div>
            }
          >
            <div class="card mt-2">
              <div class="card-body">
                <table class="table table-striped mb-0">
                  <thead>
                    <tr>
                      <th role="button" onClick={() => setSort("name")}>
                        Name{sortIndicator("name")}
                      </th>
                      <th>Type</th>
                      <th role="button" onClick={() => setSort("hostname")}>
                        Hostname{sortIndicator("hostname")}
                      </th>
                      <th role="button" onClick={() => setSort("version")}>
                        Version{sortIndicator("version")}
                      </th>
                      <th>Capabilities</th>
                      <th role="button" onClick={() => setSort("last_seen")}>
                        Last Seen{sortIndicator("last_seen")}
                      </th>
                      <th class="text-end">RTT</th>
                      <th class="text-end">Status</th>
                      <th>Key</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={sorted()}>
                      {(row) => (
                        <tr>
                          <td class="align-middle">{row.name}</td>
                          <td class="align-middle">
                            <Show
                              when={row.kind === "agent"}
                              fallback={
                                <span class="badge text-bg-secondary">
                                  Server-local
                                </span>
                              }
                            >
                              <span class="badge text-bg-primary">Agent</span>
                            </Show>
                          </td>
                          <td class="align-middle">{row.hostname ?? "—"}</td>
                          <td class="align-middle">{row.version ?? "—"}</td>
                          <td class="align-middle">
                            <For each={row.capabilities}>
                              {(capability) => (
                                <span class="badge text-bg-secondary me-1">
                                  {capability}
                                </span>
                              )}
                            </For>
                          </td>
                          <td class="align-middle">
                            {row.kind === "server"
                              ? "—"
                              : row.last_seen === undefined
                                ? "Never"
                                : parse_timestamp(row.last_seen).fromNow()}
                          </td>
                          <td class="align-middle text-end">
                            {row.rtt_ms === undefined
                              ? "—"
                              : `${row.rtt_ms} ms`}
                          </td>
                          <td class="align-middle text-end">
                            <Show
                              when={rowStatus(row) === "live"}
                              fallback={
                                <Show
                                  when={rowStatus(row) === "stale"}
                                  fallback={
                                    <span class="badge text-bg-secondary">
                                      Offline
                                    </span>
                                  }
                                >
                                  <span class="badge text-bg-warning">
                                    Stale
                                  </span>
                                </Show>
                              }
                            >
                              <span class="badge text-bg-success">Live</span>
                            </Show>
                          </td>
                          <td class="align-middle text-nowrap">
                            <Show
                              when={row.key}
                              fallback={
                                <Show
                                  when={row.kind === "agent"}
                                  fallback={<span>—</span>}
                                >
                                  <button
                                    type="button"
                                    class="btn btn-sm btn-outline-primary"
                                    disabled={creating()}
                                    onClick={() => createKey(row.name)}
                                  >
                                    Add Key
                                  </button>
                                </Show>
                              }
                            >
                              <Show
                                when={revealed()[row.key!.id] !== undefined}
                                fallback={
                                  <>
                                    <button
                                      type="button"
                                      class="btn btn-sm btn-secondary me-2"
                                      title={`Created ${parse_timestamp(
                                        row.key!.created_at,
                                      ).format("YYYY-MM-DD HH:mm")}`}
                                      onClick={() => toggleReveal(row.key!)}
                                    >
                                      Reveal
                                    </button>
                                    <button
                                      type="button"
                                      class="btn btn-sm btn-danger"
                                      onClick={() => deleteKey(row.key!)}
                                    >
                                      Delete
                                    </button>
                                  </>
                                }
                              >
                                <code class="me-2 text-break user-select-all">
                                  {revealed()[row.key!.id]}
                                </code>
                                <CopyButton text={revealed()[row.key!.id]} />
                                <button
                                  type="button"
                                  class="btn btn-sm btn-secondary ms-2 me-2"
                                  onClick={() => toggleReveal(row.key!)}
                                >
                                  Hide
                                </button>
                                <button
                                  type="button"
                                  class="btn btn-sm btn-danger"
                                  onClick={() => deleteKey(row.key!)}
                                >
                                  Delete
                                </button>
                              </Show>
                            </Show>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </>
  );
}
