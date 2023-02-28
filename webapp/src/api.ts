// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import axios, { AxiosResponse } from "axios";
import { EventWrapper } from "./types";
import Queue from "queue";
import { createSignal } from "solid-js";
import { get_timezone_offset_str } from "./datetime";

const SESSION_ID_HEADER = "x-evebox-session-id";

export const [SERVER_REVISION, SET_SERVER_REVISION] = createSignal<
  null | string
>(null);

let SESSION_ID: string | null = localStorage.getItem("SESSION_ID");

export const [QUEUE_SIZE, SET_QUEUE_SIZE] = createSignal(0);

const QUEUE = new Queue({ concurrency: 3 });

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

function setSessionId(session_id: string) {
  SESSION_ID = session_id;
  localStorage.setItem("SESSION_ID", SESSION_ID);
}

async function update_revision(response: AxiosResponse<any, any>) {
  if (response.headers && response.headers) {
    const server_rev = response!.headers["x-evebox-git-revision"];
    if (server_rev) {
      SET_SERVER_REVISION(server_rev);
    }
  }
  return response;
}

async function get(url: string, params: any = {}): Promise<any> {
  let headers = {
    "x-evebox-session-id": SESSION_ID,
  };
  return axios
    .get(url, {
      headers: headers,
      params: params,
    })
    .then(update_revision);
}

async function post(url: string, params: any = {}): Promise<any> {
  let headers = {
    "x-evebox-session-id": SESSION_ID,
  };
  return axios.post(url, params, {
    headers: headers,
  });
}

export async function getUser(): Promise<UserResponse> {
  let response = await get("api/1/user");
  return response.data;
}

export interface ConfigResponse {
  features: {
    comments: boolean;
    reporting: boolean;
  };
  defaults: {
    time_range?: string;
  };
  "event-services": any[];
  datastore: string;
}

export async function getConfig(): Promise<ConfigResponse> {
  return get("api/1/config").then((response) => response.data);
}

export async function login(
  username: string,
  password: string
): Promise<[boolean, LoginResponse]> {
  let params = new URLSearchParams({
    username: username,
    password: password,
  });

  let response = await axios.post<LoginResponse>("api/1/login", params);
  setSessionId(response.data.session_id);
  return [true, response.data];
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
}): Promise<{ events: EventWrapper[]; ecs: boolean }> {
  let params: any = {
    query_string: options?.query_string,
  };
  if (options?.time_range) {
    params.time_range = `${options.time_range}s`;
  }
  if (options?.tags) {
    params.tags = options.tags.join(",");
  }
  return get("api/1/alerts", params).then((response) => response.data);
}

export interface GetEventsOptions {
  query_string?: string;
  event_type?: string;
  time_range?: number;
  min_timestamp?: string;
  max_timestamp?: string;
  order?: "asc" | "desc";
}

export async function getEvents(
  options?: GetEventsOptions
): Promise<{ events: EventWrapper[]; esc: boolean }> {
  let params: {
    tz_offset?: string;
    time_range?: string;
    event_type?: string;
    query_string?: string;
    order?: string;
    min_timestamp?: string;
    max_timestamp?: string;
  } = {
    tz_offset: get_timezone_offset_str(),
  };
  if (options?.time_range) {
    params.time_range = `${options.time_range}s`;
  }
  if (options?.event_type) {
    params.event_type = options.event_type;
  }
  if (options?.query_string) {
    params.query_string = options.query_string;
  }
  if (options?.order) {
    params.order = options.order;
  }
  if (options?.min_timestamp) {
    params.min_timestamp = options.min_timestamp;
  }
  if (options?.max_timestamp) {
    params.max_timestamp = options.max_timestamp;
  }
  return get("api/1/events", params).then((response) => response.data);
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
    return post("api/1/alert-group/archive", params);
  });
}

export async function archiveEvent(event: EventWrapper): Promise<any> {
  return queueAdd(() => {
    return post(`api/1/event/${event._id}/archive`);
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
    return post("api/1/alert-group/star", params);
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
    return post("api/1/alert-group/unstar", params);
  });
}

export async function getEventById(id: string): Promise<EventWrapper> {
  return get(`api/1/event/${id}`).then((response) => response.data);
}

export async function getVersion(): Promise<{
  revision: string;
  version: string;
}> {
  return get("api/1/version").then((response) => response.data);
}

export interface LoginOptions {
  authentication: {
    required: boolean;
    type: string;
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

export async function statsAgg(
  field: string,
  differential: boolean = false,
  time_range?: number,
  sensor_name?: string
): Promise<StatsAggResponse> {
  let url;
  if (differential) {
    url = "api/1/stats/agg/diff";
  } else {
    url = "api/1/stats/agg";
  }
  return get(url, {
    field: field,
    time_range: time_range,
    sensor_name: sensor_name,
  }).then((response) => response.data);
}

export async function getSensors(): Promise<{ data: string[] }> {
  return get("api/1/sensors").then((response) => response.data);
}

export interface GroupByQueryRequest {
  field: string;
  time_range?: string;
  size?: number;
  order?: "asc" | "desc";
  q?: string;
}

export interface GroupByQueryResponse {
  rows: GroupByQueryResponseRow[];
}

export interface GroupByQueryResponseRow {
  count: number;
  key: any;
}

class Api {
  async groupBy(request: GroupByQueryRequest): Promise<GroupByQueryResponse> {
    return get("api/1/groupby", request).then((response) => response.data);
  }

  async histogramTime(request: {
    time_range: string;
    interval?: string;
    event_type: string;
    query_string?: string;
  }): Promise<{ data: { count: number; time: number }[] }> {
    return get("api/1/report/histogram/time", request).then(
      (response) => response.data
    );
  }

  async getSensors(): Promise<{ data: string[] }> {
    return get("api/1/sensors").then((response) => response.data);
  }

  escalateAggregateAlert(alert: EventWrapper) {
    const params = {
      signature_id: alert._source.alert!.signature_id,
      src_ip: alert._source.src_ip,
      dest_ip: alert._source.dest_ip,
      min_timestamp: alert._metadata?.min_timestamp,
      max_timestamp: alert._metadata?.max_timestamp,
    };
    return queueAdd(() => {
      return post("api/1/alert-group/star", params);
    });
  }

  deEscalateAggregateAlert(alert: EventWrapper) {
    const params = {
      signature_id: alert._source.alert!.signature_id,
      src_ip: alert._source.src_ip,
      dest_ip: alert._source.dest_ip,
      min_timestamp: alert._metadata?.min_timestamp,
      max_timestamp: alert._metadata?.max_timestamp,
    };
    return queueAdd(() => {
      return post("api/1/alert-group/unstar", params);
    });
  }

  escalateEvent(event: EventWrapper) {
    return post(`api/1/event/${event._id}/escalate`);
  }

  deEscalateEvent(event: EventWrapper) {
    return post(`api/1/event/${event._id}/de-escalate`);
  }

  dhcpAck(query: { time_range?: string; sensor?: string }): Promise<any> {
    return get(`api/1/dhcp/ack`, query).then((response) => response.data);
  }

  eventToPcap(event: EventWrapper, what: "packet" | "payload") {
    // Set a cook with the session key to expire in 60 seconds from now.
    const expires = new Date(new Date().getTime() + 60000);
    const cookie = `${SESSION_ID_HEADER}=${SESSION_ID}; expires=${expires.toUTCString()}`;
    console.log("Setting cookie: " + cookie);
    document.cookie = cookie;

    const form = document.createElement("form") as HTMLFormElement;
    form.setAttribute("method", "post");
    form.setAttribute("action", "api/1/eve2pcap");

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
}

export const API = new Api();
