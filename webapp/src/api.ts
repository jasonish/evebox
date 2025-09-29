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

const QUEUE = new Queue({ concurrency: 9 });

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
  QUEUE.start();
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
  defaults: {
    time_range?: string;
  };
  "event-services": any[];
  datastore: string;
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
}

export async function statsAgg(
  field: string,
  differential: boolean = false,
  time_range?: number,
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
    time_range: time_range,
    sensor_name: sensor_name,
  }).then((response) => response.data);
}

export async function statsAggBySensor(
  field: string,
  differential: boolean = false,
  time_range?: number,
): Promise<StatsAggBySensorResponse> {
  let url;
  if (differential) {
    url = "api/stats/agg/diff/by-sensor";
  } else {
    url = "api/stats/agg/by-sensor";
  }
  return get(url, {
    field: field,
    time_range: time_range,
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
}
