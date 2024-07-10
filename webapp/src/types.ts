// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

export interface Event {
  _id: string;
  _source: EventSource;
}

export interface EventWrapper {
  _id: string;
  _source: EventSource;
  _metadata?: EventWrapperMetadata;

  __private: {
    selected: boolean;
  };
}

export interface AggregateAlert extends EventWrapper {
  _metadata: EventWrapperMetadata;
}

export interface EventWrapperMetadata {
  count: number;
  escalated_count: number;
  min_timestamp: string;
  max_timestamp: string;
}

export interface EventSource {
  "@timestamp": string;
  timestamp: string;
  src_ip: string;
  dest_ip: string;
  event_type: string;
  proto: string;
  src_port?: number;
  dest_port?: number;
  host?: string | any;
  tags?: string[];
  in_iface?: string;
  flow_id: number;
  community_id?: string;
  payload: string;
  packet: string;
  alert?: EveAlert;
  tls?: EveTls;
  flow?: EveFlow;
  netflow?: EveNetflow;
  dns?: EveDns;
  http?: EveHttp;
  app_proto?: string;
  stats?: EveStats;
  smb?: EveSmb;
  ssh?: EveSsh;
  fileinfo?: EveFileinfo;
  http2?: EveHttp2;
  anomaly?: EveAnomaly;
  dhcp?: EveDhcp;
  engine?: EveEngine;

  // log_level only appears to exist for "engine" events.
  log_level?: string;

  // ECS source and destination including GeoIP.
  source?: EcsAddress;
  destination?: EcsAddress;

  // EveBox GeoIP fields.
  geoip_source?: EcsGeo;
  geoip_destination?: EcsGeo;

  geoip?: SelksGeo;

  // Allow index by string key...
  [index: string]: any;
}

export interface EcsAddress {
  address: string;
  geo: EcsGeo;
}

export interface EcsGeo {
  city_name?: string;
  continent_name?: string;
  country_name?: string;
  region_name?: string;
}

// The GeoIP format used in SELKS.  This might be standard Logstash format as well.
export interface SelksGeo {
  continent_code: string;
  country_code2: string;
  country_code3: string;
  country_name: string;
  ip: string;
  location: {
    lat: number;
    lon: number;
  };
  longitude: number;
  timezone: string;
}

export interface EveAlert {
  signature: string;
  signature_id: number;
  severity: number;
  rev: number;
  gid: number;
  category: string;
  rule: string;
  action: string;
}

export interface EveTls {
  ja3: { [key: string]: string };
  ja3s: { [key: string]: string };
  session_resumed: boolean;
  sni: string;
  version: string;
  fingerprint: string;
  issuerdn: string;
  notafter: string;
  notbefore: string;
  serial: string;
  subject: string;
}

export interface EveFlow {
  age: number;
  alerted: boolean;
  bytes_toclient: number;
  bytes_toserver: number;
  end: string;
  pkts_toclient: number;
  pkts_toserver: number;
  reason: string;
  start: string;
  state: string;
}

export interface EveNetflow {
  age: number;
  bytes: number;
  start: string;
  end: string;
  max_ttl: number;
  min_ttl: number;
  pkts: number;
}

export interface EveDns {
  id: number;
  rrname: string;
  rrtype: string;
  tx_id: number;
  type: string;
  rcode: string;

  query?: {
    rrname: string;
  }[];

  queries: {
    rrname: string;
    rrtype: string;
  }[];

  answers?: {
    rdata: string;
    rrname: string;
    rrtype: string;
    ttl: number;
  }[];

  authorities?: {
    rrname: string;
    rrtype: string;
    ttl: number;
    soa?: {
      mname: string;
      rname: string;
    };
  }[];
}

export interface EveHttp {
  hostname: string;
  http_method: string;
  http_port: number;
  length: number;
  protocol: string;
  status: number;
  url: string;
  http_content_type: string;
}

export interface EveStats {
  uptime: number;
  capture: {
    kernel_drops: number;
    kernel_packets: number;
  };
  decoder: {
    bytes: number;
    pkts: number;
  };
  detect: {
    alert: number;
  };
}

export interface EveSmb {
  command: string;
  dialect: string;
  status: string;

  [index: string]: any;
}

export interface EveSsh {
  client: {
    proto_version: string;
    software_version: string;
  };
  server: {
    proto_version: string;
    software_version: string;
  };
}

export interface EveFileinfo {
  filename: string;
  state: string;
  stored: string;
  size: number;
}

export interface EveHttp2 {
  request_headers: { name: string; value: string }[];
  response_headers: { name: string; value: string }[];
}

export interface EveAnomaly {
  app_proto: string;
  event: string;
  layer: string;
  type: string;
}

export interface EveDhcp {
  assigned_ip: string;
  client_ip: string;
  client_mac: string;
  dhcp_type: string;
  dns_server: string[];
  hostname: string;
  id: number;
  lease_time: number;
  next_server_ip: number;
  rebinding_time: number;
  relay_ip: string;
  renewal_time: number;
  routers: string[];
  subnet_mask: string[];
  type: string;
}

export interface EveEngine {
  message: string;
  module: string;
  thread_name: string;
}
