// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import axios, { AxiosResponse } from "axios";
import { EventWrapper } from "./types";
import Queue from "queue";
import { createSignal } from "solid-js";
import { get_timezone_offset_str } from "./datetime";
import { SET_IS_AUTHENTICATED } from "./global";

export const [SERVER_REVISION, SET_SERVER_REVISION] = createSignal<
  null | string
>(null);

export const [QUEUE_SIZE, SET_QUEUE_SIZE] = createSignal(0);

const QUEUE = new Queue({ concurrency: 9, autostart: true });

function queueAdd(func: any): Promise<any> {
  const p = new Promise<any>((resolve, reject) => {
    QUEUE.push((cb: any) => {
      func()
        .then(() => {
          cb();
          resolve(null);
        })
        .catch(() => {
          cb();
          reject();
        })
        .finally(() => {
          SET_QUEUE_SIZE(QUEUE.length);
        });
    });
  });
  SET_QUEUE_SIZE(QUEUE.length);
  return p;
}

async function update_revision(response: AxiosResponse<any, any>) {
  if (response && response.headers) {
    const server_rev = response!.headers["x-evebox-git-revision"];
    if (server_rev) {
      SET_SERVER_REVISION(server_rev);
    }
  }
  return response;
}

export async function get(url: string, params: any = {}): Promise<any> {
  return axios
    .get(url, {
      params: params,
    })
    .then(update_revision)
    .catch((error) => {
      if (error && error.rsponse && error.response.status === 401) {
        SET_IS_AUTHENTICATED(false);
      }
      throw error;
    });
}

export async function post(url: string, params: any = {}): Promise<any> {
  return axios.post(url, params, {});
}

async function postJson(url: string, body: any = {}): Promise<any> {
  return axios.post(url, body, {});
}

export async function postComment(
  eventId: string | number,
  comment: string,
): Promise<any> {
  let body = {
    comment: comment,
  };
  return postJson(`api/event/${eventId}/comment`, body);
}

export async function getUser(): Promise<UserResponse> {
  let response = await get("api/user");
  return response.data;
}

export interface ConfigResponse {
  mode: "server" | "oneshot";
  defaults: {
    time_range?: string;
  };
  "event-services": any[];
  datastore: string;
  // For Elasticsearch datastores, the concrete distribution: "elasticsearch"
  // or "opensearch". Null/undefined for other datastores.
  distribution?: string | null;
  // Server-side pcap defaults, so the download UI can pre-fill its max
  // size. Null on platforms where pcap is compiled out (Windows).
  pcap?: {
    max_size_bytes?: number;
  } | null;
}

export async function getConfig(): Promise<ConfigResponse> {
  return get("api/config").then((response) => response.data);
}

export async function login(
  username: string,
  password: string,
): Promise<[boolean, LoginResponse]> {
  let params = new URLSearchParams({
    username: username,
    password: password,
  });

  let response = await axios.post<LoginResponse>("api/login", params);
  return [true, response.data];
}

export async function logout() {
  let _response = await post("api/logout");
  SET_IS_AUTHENTICATED(false);
}

export interface AlertsResponse {
  events: EventWrapper[];
  ecs: boolean;
  took: number;
  timed_out: boolean;
}

export async function alerts(options?: {
  // A query string to apply to the alert search.
  query_string?: string;
  // Time range, a value in seconds.
  time_range?: number;
  // Tags that must be present.
  tags?: string[];
  // Tags that must not be present.
  not_tags?: string[];
  sensor: string | undefined;
  timeout: undefined | number;
}): Promise<AlertsResponse> {
  let params: any = {
    query_string: options?.query_string,
  };
  if (options?.time_range) {
    params.time_range = `${options.time_range}s`;
  }
  if (options?.tags) {
    params.tags = options.tags.join(",");
  }
  if (options?.sensor) {
    params.sensor = options.sensor;
  }
  if (options?.timeout) {
    params.timeout = options.timeout;
  }
  return get("api/alerts", params).then((response) => response.data);
}

export interface EventsQueryParams {
  event_type?: string;
  to?: string;
  from?: string;
  order?: "asc" | "desc";
  sensor?: string;
  query_string?: string;
  tz_offset?: string;
}

export async function getEvents(
  params?: EventsQueryParams,
): Promise<{ events: EventWrapper[]; esc: boolean }> {
  if (!params) {
    params = {};
  }
  if (!params?.tz_offset) {
    params.tz_offset = get_timezone_offset_str();
  }
  return get("api/events", params).then((response) => response.data);
}

export async function archiveAggregateAlert(alert: EventWrapper) {
  const params = {
    signature_id: alert._source.alert!.signature_id,
    src_ip: alert._source.src_ip,
    dest_ip: alert._source.dest_ip,
    min_timestamp: alert._metadata?.min_timestamp,
    max_timestamp: alert._metadata?.max_timestamp,
  };
  return queueAdd(() => {
    return post("api/alert-group/archive", params);
  });
}

export async function archiveEvent(event: EventWrapper): Promise<any> {
  return queueAdd(() => {
    return post(`api/event/${event._id}/archive`);
  });
}

export async function escalateAggregateAlert(alert: EventWrapper) {
  const params = {
    signature_id: alert._source.alert!.signature_id,
    src_ip: alert._source.src_ip,
    dest_ip: alert._source.dest_ip,
    min_timestamp: alert._metadata?.min_timestamp,
    max_timestamp: alert._metadata?.max_timestamp,
  };
  return queueAdd(() => {
    return post("api/alert-group/star", params);
  });
}

export async function unescalateAggregateAlert(alert: EventWrapper) {
  const params = {
    signature_id: alert._source.alert!.signature_id,
    src_ip: alert._source.src_ip,
    dest_ip: alert._source.dest_ip,
    min_timestamp: alert._metadata?.min_timestamp,
    max_timestamp: alert._metadata?.max_timestamp,
  };
  return queueAdd(() => {
    return post("api/alert-group/unstar", params);
  });
}

export async function getEventById(id: string): Promise<EventWrapper> {
  return get(`api/event/${id}`).then((response) => response.data);
}

export async function getVersion(): Promise<{
  revision: string;
  version: string;
}> {
  return get("api/version").then((response) => response.data);
}

export interface UpdateManifest {
  version: string;
}

// Default URL of the release manifest, served with permissive CORS so the
// browser can fetch it directly without going through the EveBox server.
export const UPDATE_MANIFEST_URL =
  "https://evebox.org/files/release/latest.json";

// Fetch the release manifest directly from the download host. This deliberately
// does not go through the EveBox server or the axios instance.
export async function getUpdateManifest(
  url: string = UPDATE_MANIFEST_URL,
): Promise<UpdateManifest> {
  const response = await fetch(url, {
    headers: { Accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`);
  }
  return response.json();
}

export interface LoginOptions {
  authentication: {
    required: boolean;
  };
}

export interface LoginResponse {
  session_id: string;
}

export interface UserResponse {
  username: string;
}

export interface StatsAggResponse {
  data: { timestamp: string; value: number }[];
}

export interface StatsAggBySensorResponse {
  data: { [sensor: string]: { timestamp: string; value: number }[] };
  min_timestamp?: string;
  max_timestamp?: string;
}

export async function statsAgg(
  field: string,
  differential: boolean = false,
  min_timestamp?: string,
  max_timestamp?: string,
  sensor_name?: string,
): Promise<StatsAggResponse> {
  let url;
  if (differential) {
    url = "api/stats/agg/diff";
  } else {
    url = "api/stats/agg";
  }
  return get(url, {
    field: field,
    min_timestamp: min_timestamp,
    max_timestamp: max_timestamp,
    sensor_name: sensor_name,
  }).then((response) => response.data);
}

export async function statsAggBySensor(
  field: string,
  differential: boolean = false,
  min_timestamp?: string,
  max_timestamp?: string,
): Promise<StatsAggBySensorResponse> {
  let url;
  if (differential) {
    url = "api/stats/agg/diff/by-sensor";
  } else {
    url = "api/stats/agg/by-sensor";
  }
  return get(url, {
    field: field,
    min_timestamp: min_timestamp,
    max_timestamp: max_timestamp,
  }).then((response) => response.data);
}

export async function getSensors(): Promise<{ data: string[] }> {
  return get("api/sensors").then((response) => response.data);
}

export interface AggRequest {
  field: string;
  time_range?: string;
  size?: number;
  order?: "asc" | "desc";
  q?: string;
}

export interface AggResponse {
  rows: AggResponseRow[];
}

export interface AggResponseRow {
  count: number;
  key: any;
}

export async function fetchAgg(request: AggRequest): Promise<AggResponse> {
  return get("api/agg", request).then((response) => response.data);
}

export async function dhcpAck(query: {
  time_range?: string;
  sensor?: string;
}): Promise<any> {
  const response = await get(`api/dhcp/ack`, query);
  return response.data;
}

export async function dhcpRequest(query: {
  time_range?: string;
  sensor?: string;
}): Promise<any> {
  const response = await get(`api/dhcp/request`, query);
  return response.data;
}

export namespace API {
  export async function getJson(url: string): Promise<any> {
    let response = await fetch(url);
    let json = response.json();
    return json;
  }

  export async function postJson(url: string, body: any): Promise<Response> {
    return await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
  }

  export async function histogramTime(request: {
    time_range: string;
    interval?: string;
    event_type: string;
    query_string?: string;
  }): Promise<{ data: { count: number; time: number }[] }> {
    return get("api/report/histogram/time", request).then(
      (response) => response.data,
    );
  }

  export async function getSensors(): Promise<{ data: string[] }> {
    return get("api/sensors").then((response) => response.data);
  }

  export async function getEventTypes(request: {
    time_range?: string;
  }): Promise<string[]> {
    return get("api/event_types", request).then((response) => response.data);
  }

  export async function escalateAggregateAlert(alert: EventWrapper) {
    const params = {
      signature_id: alert._source.alert!.signature_id,
      src_ip: alert._source.src_ip,
      dest_ip: alert._source.dest_ip,
      min_timestamp: alert._metadata?.min_timestamp,
      max_timestamp: alert._metadata?.max_timestamp,
    };
    return queueAdd(() => {
      return post("api/alert-group/star", params);
    });
  }

  export async function deEscalateAggregateAlert(alert: EventWrapper) {
    const params = {
      signature_id: alert._source.alert!.signature_id,
      src_ip: alert._source.src_ip,
      dest_ip: alert._source.dest_ip,
      min_timestamp: alert._metadata?.min_timestamp,
      max_timestamp: alert._metadata?.max_timestamp,
    };
    return queueAdd(() => {
      return post("api/alert-group/unstar", params);
    });
  }

  export async function escalateEvent(event: EventWrapper) {
    return post(`api/event/${event._id}/escalate`);
  }

  export async function deEscalateEvent(event: EventWrapper) {
    return post(`api/event/${event._id}/de-escalate`);
  }

  export async function eventToPcap(
    event: EventWrapper,
    what: "packet" | "payload",
  ) {
    const form = document.createElement("form") as HTMLFormElement;
    form.setAttribute("method", "post");
    form.setAttribute("action", "api/eve2pcap");

    const whatField = document.createElement("input") as HTMLElement;
    whatField.setAttribute("type", "hidden");
    whatField.setAttribute("name", "what");
    whatField.setAttribute("value", what);
    form.appendChild(whatField);

    const eventField = document.createElement("input") as HTMLElement;
    eventField.setAttribute("type", "hidden");
    eventField.setAttribute("name", "event");
    eventField.setAttribute("value", JSON.stringify(event._source));
    form.appendChild(eventField);

    document.body.appendChild(form);
    form.submit();
  }

  // A structured error from POST /api/pcap ({"error": {...}}).
  export class PcapError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.name = "PcapError";
      this.code = code;
    }
  }

  // The pre-flight result from GET /api/pcap/validate.
  export interface PcapValidation {
    ok: boolean;
    // The download filename the server will set.
    filename: string;
  }

  // Parameters shared by buffered POST /api/pcap and native GET
  // /api/pcap. Every field is optional; the server resolves the window
  // and filter from whichever are present (see the pcap query-builder
  // contract):
  //   - eventId: derive from a stored event; absent for standalone.
  //   - filter: raw libpcap BPF. A non-empty string is that filter; an
  //     empty string means all packets in the window. Omit the field
  //     to leave it unset.
  //   - start + duration: free-form absolute window (start is RFC3339,
  //     duration is a "1m"/"5m" style string).
  //   - before + after: event-relative window ("1m" style), needs an
  //     event.
  //   - maxSize: per-request output cap ("200mb", "1gb", bare bytes, or
  //     "unlimited"/"0"). Native GET may raise or lift the server
  //     default; buffered POST may only keep or lower it.
  //   - source: explicit local/agent source name. Normally omitted because
  //     event identity or the sole available source resolves it.
  export interface PcapRequestParams {
    eventId?: string;
    filter?: string;
    start?: string;
    duration?: string;
    before?: string;
    after?: string;
    maxSize?: string;
    source?: string;
  }

  // Build the server's snake_case query params from the camelCase
  // request, including only fields that were actually provided. An
  // empty-string filter is meaningful (all packets in the window), so
  // it is dropped only when undefined.
  function pcapParams(params: PcapRequestParams): URLSearchParams {
    const q = new URLSearchParams();
    if (params.eventId !== undefined) q.set("event_id", params.eventId);
    if (params.filter !== undefined) q.set("filter", params.filter);
    if (params.start !== undefined) q.set("start", params.start);
    if (params.duration !== undefined) q.set("duration", params.duration);
    if (params.before !== undefined) q.set("before", params.before);
    if (params.after !== undefined) q.set("after", params.after);
    if (params.maxSize !== undefined) q.set("max_size", params.maxSize);
    if (params.source !== undefined) q.set("source", params.source);
    return q;
  }

  // Parse a failed pcap request's structured error body
  // ({"error": {...}}) into a PcapError. A user abort fired while the
  // body is read is re-thrown untouched so the caller's cancel path
  // stays silent.
  async function pcapError(response: Response): Promise<PcapError> {
    if (response.status === 401) {
      SET_IS_AUTHENTICATED(false);
    }
    let code = "error";
    let message = `PCAP request failed (${response.status}).`;
    try {
      const json = await response.json();
      if (json?.error) {
        code = json.error.code ?? code;
        message = json.error.message ?? message;
      }
    } catch (e: any) {
      if (e?.name === "AbortError") {
        throw e;
      }
    }
    return new PcapError(code, message);
  }

  // A selectable pcap source: the server-local spool (kind "server",
  // always named "(server)") or a connected agent (kind "agent").
  export interface PcapSource {
    name: string;
    kind: string;
  }

  // The pcap sources a request's `source` parameter may select right
  // now. Used to populate the custom download form's source picker.
  // Failures carry the HTTP status so callers can tell a missing route
  // (404, builds without pcap support) from a real error.
  export async function getPcapSources(
    signal?: AbortSignal,
  ): Promise<PcapSource[]> {
    const response = await fetch("api/pcap/sources", { signal: signal });
    if (!response.ok) {
      const error: any = new Error(
        `Failed to fetch pcap sources: ${response.status}`,
      );
      error.status = response.status;
      throw error;
    }
    const json = await response.json();
    return json.sources ?? [];
  }

  // One row from GET /api/agents: a connected remote agent. The
  // server-local pcap spool is not an agent and does not appear here;
  // getPcapSources() is what knows whether one is configured.
  export interface AgentInfo {
    name: string;
    hostname: string;
    version: string;
    capabilities: string[];
    connected_at: string;
    last_seen: string;
    rtt_ms?: number;
  }

  export async function getAgents(): Promise<AgentInfo[]> {
    const response = await fetch("api/agents");
    if (!response.ok) {
      throw new Error(`Failed to fetch agents: ${response.status}`);
    }
    return await response.json();
  }

  // Pre-flight a pcap request: GET /api/pcap/validate performs structural
  // validation without extracting packets. Errors that require opening a
  // capture (including malformed BPF) are surfaced by startPcapDownload.
  // An abort via `signal` rejects with the original AbortError.
  export async function validatePcap(
    params: PcapRequestParams,
    signal?: AbortSignal,
  ): Promise<PcapValidation> {
    const response = await fetch(`api/pcap/validate?${pcapParams(params)}`, {
      signal: signal,
    });
    if (!response.ok) {
      throw await pcapError(response);
    }
    return await response.json();
  }

  // Hand the byte transfer to the browser through a same-origin iframe.
  // Successful Content-Disposition responses stream straight to disk with no
  // in-memory buffering. A structured error instead loads in the hidden frame,
  // where it can be turned into the same toast as a pre-flight failure rather
  // than being saved under a .pcap filename.
  export function startPcapDownload(
    params: PcapRequestParams,
    onError: (error: PcapError) => void,
  ): void {
    const frame = document.createElement("iframe");
    frame.hidden = true;
    frame.setAttribute("aria-hidden", "true");
    frame.addEventListener("load", () => {
      let error = new PcapError("error", "PCAP request failed.");
      try {
        const text = frame.contentDocument?.body?.textContent;
        const json = text ? JSON.parse(text) : undefined;
        if (json?.error) {
          error = new PcapError(
            json.error.code ?? "error",
            json.error.message ?? "PCAP request failed.",
          );
        }
      } catch {
        // A non-JSON response still becomes a generic in-app error rather than
        // replacing the page or being mislabeled as a packet capture.
      }
      frame.remove();
      onError(error);
    });
    frame.src = `api/pcap?${pcapParams(params)}`;
    document.body.appendChild(frame);

    // Successful attachment navigations do not fire the iframe load event.
    // Keep the frame alive while the browser owns the transfer, then discard
    // the otherwise inert DOM node.
    window.setTimeout(() => frame.remove(), 5 * 60 * 1000);
  }

  // Markers the server sets on a successful buffered pcap response so
  // the caller can inform the user.
  export interface PcapResult {
    // Capture limits stopped extraction before the end of the flow.
    truncated: boolean;
  }

  function filenameFromDisposition(header: string | null): string | undefined {
    if (!header) return undefined;
    const match = /filename=([^;]+)/.exec(header);
    if (!match) return undefined;
    return match[1].trim().replace(/^"|"$/g, "");
  }

  // Buffered pcap download for the quick event button: POST /api/pcap,
  // read the whole (server-limited) capture into memory, then trigger a
  // browser download. Unlike the native streaming download this reads
  // the response, so truncation and structured errors (no-match, ...)
  // surface to the caller. Throws PcapError on a structured API error;
  // an abort via `signal` rejects with the original AbortError.
  export async function fetchPcap(
    eventId: string,
    signal?: AbortSignal,
  ): Promise<PcapResult> {
    const response = await fetch("api/pcap", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ event_id: eventId }),
      signal: signal,
    });
    if (!response.ok) {
      throw await pcapError(response);
    }
    const truncated = response.headers.get("x-evebox-pcap-truncated") !== null;
    let blob: Blob;
    try {
      blob = await response.blob();
    } catch (err: any) {
      // A user-initiated abort surfaces here as an AbortError; pass it
      // through untouched. Anything else (a TypeError) means the server
      // aborted the stream mid-transfer.
      if (err?.name === "AbortError") {
        throw err;
      }
      throw new PcapError(
        "interrupted",
        "PCAP download was interrupted before completing.",
      );
    }
    // A truncated response with an empty (or header-only) body means the
    // limits kicked in before any packet matched: nothing to download.
    if (truncated && blob.size <= 24) {
      throw new PcapError(
        "truncated-empty",
        "Capture limits stopped extraction before any matching packet.",
      );
    }
    const filename =
      filenameFromDisposition(response.headers.get("content-disposition")) ??
      `${eventId}.pcap`;
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    // Revoking synchronously can abort the just-started download on some
    // browsers (Safari); give it plenty of time.
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
    return { truncated: truncated };
  }

  let ES_TRACKER: EventSource[] = [];

  export async function cancelAllSse() {
    while (ES_TRACKER.length > 0) {
      const es = ES_TRACKER.pop();
      if (es) {
        es.close();
      }
    }
  }

  export async function getSseAgg(
    params: any,
    version: () => number,
    onData?: any,
  ): Promise<void> {
    return new Promise((resolve, _reject) => {
      const currentVersion = version();
      let urlSearchParams = new URLSearchParams(Object.entries(params));
      let url = `api/sse/agg?${urlSearchParams.toString()}`;
      const es = new EventSource(url);
      ES_TRACKER.push(es);
      es.onmessage = (e) => {
        if (currentVersion != version()) {
          console.log("SSE version invalidated, closing");
          es.close();
          return;
        }
        const data = JSON.parse(e.data);
        if (onData) {
          onData(data);
        }
      };

      es.onerror = () => {
        es.close();
        if (currentVersion == version()) {
          if (onData) {
            onData(null);
          }
        }

        const index = ES_TRACKER.indexOf(es);
        if (index > -1) {
          ES_TRACKER.splice(index, 1);
        }

        resolve();
      };
    });
  }

  export interface AddAutoArchiveRequest {
    sensor?: string;
    src_ip?: string;
    dest_ip?: string;
    signature_id: number;
    comment?: string;
  }

  export async function addAutoArchive(
    params: AddAutoArchiveRequest,
  ): Promise<any> {
    let urlSearchParams = new URLSearchParams(Object.entries(params));
    return fetch("api/admin/filter/add", {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        Accept: "application/json",
      },
      body: urlSearchParams.toString(),
    });
  }

  export async function fetchFilters(): Promise<any> {
    let response = await fetch("api/admin/filters");
    if (response.ok) {
      let json = await response.json();
      return json;
    }
  }

  export async function doDelete(path: string): Promise<any> {
    let response = await fetch(path, {
      method: "DELETE",
    });

    if (!response.ok) {
      throw new Error(
        `HTTP error! status: ${response.status} - ${response.statusText}`,
      );
    }

    return response;
  }

  export async function deleteFilter(id: number): Promise<any> {
    return doDelete(`api/admin/filter/${id}`);
  }

  // One row from GET /api/agents/keys. The key value itself is only
  // carried by the create and reveal responses.
  export interface AgentKeyInfo {
    id: number;
    name: string;
    created_at: string;
    last_seen: string | null;
  }

  export interface AgentKeyWithSecret extends AgentKeyInfo {
    key: string;
  }

  // The agent key endpoints report failures as {"error": "message"}.
  async function agentKeyError(response: Response): Promise<Error> {
    let message = `Request failed (${response.status})`;
    try {
      const json = await response.json();
      if (typeof json?.error === "string") {
        message = json.error;
      }
    } catch (_e) {
      // Keep the status-based message.
    }
    return new Error(message);
  }

  export async function getAgentKeys(): Promise<AgentKeyInfo[]> {
    const response = await fetch("api/agents/keys");
    if (!response.ok) {
      throw await agentKeyError(response);
    }
    return await response.json();
  }

  export async function addAgentKey(name: string): Promise<AgentKeyWithSecret> {
    const response = await fetch("api/agents/keys", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ name: name }),
    });
    if (!response.ok) {
      throw await agentKeyError(response);
    }
    return await response.json();
  }

  export async function revealAgentKey(
    id: number,
  ): Promise<AgentKeyWithSecret> {
    const response = await fetch(`api/agents/keys/${id}`);
    if (!response.ok) {
      throw await agentKeyError(response);
    }
    return await response.json();
  }

  export async function deleteAgentKey(id: number): Promise<void> {
    const response = await fetch(`api/agents/keys/${id}`, {
      method: "DELETE",
    });
    if (!response.ok) {
      throw await agentKeyError(response);
    }
  }
}
