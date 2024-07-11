// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Event } from "./types";
import { get_duration } from "./datetime";

export function formatEventDescription(event: Event): string {
  switch (event._source.event_type) {
    case "alert": {
      const alert = event._source.alert!;
      if (alert.signature) {
        return `${alert.signature}`;
      } else {
        return `[${alert.gid}:${alert.signature_id}:${alert.rev}] (${alert.category})`;
      }
    }
    case "anomaly": {
      const anom = event._source.anomaly!;
      if (anom.type === "applayer") {
        return `${anom.app_proto} - ${anom.event}`;
      } else if (anom.type === "stream") {
        return `STREAM: ${anom.event}`;
      } else if (anom.type === "decode") {
        return `DECODE: ${anom.event}`;
      }
      return JSON.stringify(anom);
    }
    case "arp": {
      const arp = event._source.arp!;
      if (arp.opcode == "request") {
        return `Request who-has ${arp.dest_ip} tell ${arp.src_ip}`;
      } else if (arp.opcode == "reply") {
        return `Reply ${arp.src_ip} is at ${arp.src_mac}`;
      }
      return JSON.stringify(arp);
    }
    case "dhcp": {
      const dhcp = event._source.dhcp!;
      let parts = [dhcp.type.toUpperCase()];
      if (dhcp.hostname) {
        parts.push(`Hostname: ${dhcp.hostname}`);
      }
      if (dhcp.assigned_ip && dhcp.assigned_ip != "0.0.0.0") {
        parts.push(`Assigned-IP: ${dhcp.assigned_ip}`);
      }
      if (dhcp.client_ip) {
        parts.push(`Client-IP: ${dhcp.client_ip}`);
      }
      return parts.join(" ");
    }
    case "dns": {
      const dns = event._source.dns!;
      let parts = [dns.type.toUpperCase()];

      if (dns.queries && dns.queries[0]) {
        parts.push(dns.queries[0].rrtype);
        parts.push(dns.queries[0].rrname);
      } else if (dns.rrname) {
        parts.push(dns.rrtype);
        parts.push(dns.rrname);
      }

      if (dns.rcode && dns.rcode !== "NOERROR") {
        parts.push(...["-", dns.rcode]);
      }
      return parts.join(" ");
    }
    case "drop": {
      const source = formatAddressWithPort(
        event._source.src_ip,
        event._source.src_port
      );
      const dest = formatAddressWithPort(
        event._source.dest_ip,
        event._source.dest_port
      );
      return `${source} => ${dest}`;
    }
    case "engine": {
      let parts = [];
      if (event._source.log_level) {
        parts.push(event._source.log_level.toUpperCase());
      }
      if (event._source.engine?.message) {
        parts.push(event._source.engine.message);
      }
      return parts.join(" ");
    }
    case "fileinfo": {
      const fileinfo = event._source.fileinfo;
      let parts = [];
      parts.push(fileinfo?.filename);

      if (event._source.http2) {
        const http2 = event._source.http2;
        for (const header of http2.request_headers) {
          if (header.name === ":authority") {
            parts.push(`Authority:${header.value}`);
          } else if (header.name === ":path") {
            parts.push(`Path:${header.value}`);
          }
        }
      }

      if (event._source.http) {
        const http = event._source.http;
        if (http.hostname) {
          parts.push(`Hostname:${http.hostname}`);
        }
        if (http.url) {
          parts.push(`Path:${http.url}`);
        }
        if (http.http_content_type) {
          parts.push(`Content-Type:${http.http_content_type}`);
        }
      }
      return parts.join(" ");
    }
    case "flow": {
      const packets =
        event._source.flow!.pkts_toclient + event._source.flow!.pkts_toserver;
      const bytes =
        event._source.flow!.bytes_toclient + event._source.flow!.bytes_toserver;
      const source = formatAddressWithPort(
        event._source.src_ip,
        event._source.src_port
      );
      const dest = formatAddressWithPort(
        event._source.dest_ip,
        event._source.dest_port
      );
      let parts = [
        event._source.proto,
        `${source} => ${dest}`,
        `Age=${event._source.flow?.age}`,
        `Packets=${packets}`,
        `Bytes=${bytes}`,
      ];
      return parts.join(" ");
    }
    case "netflow": {
      const netflow = event._source.netflow!;
      formatAddress(event._source.src_ip);
      const source = formatAddressWithPort(
        event._source.src_ip,
        event._source.src_port
      );
      const dest = formatAddressWithPort(
        event._source.dest_ip,
        event._source.dest_port
      );
      let parts = [
        event._source.proto,
        `${source} => ${dest}`,
        `Age=${netflow.age}`,
        `Packets=${netflow.pkts}`,
        `Bytes=${netflow.bytes}`,
      ];
      return parts.join(" ");
    }
    case "tls": {
      const tls = event._source.tls!;
      let parts = [];
      if (tls.version) {
        parts.push(tls.version);
      } else {
        parts.push("TLS");
      }
      if (tls.sni) {
        parts.push(tls.sni);
      }
      if (tls.subject) {
        parts.push(tls.subject);
      }
      return parts.join(" - ");
    }
    case "http": {
      const http = event._source.http!;
      let parts = [];
      if (http.http_method) {
        parts.push(http.http_method);
      }
      if (http.hostname) {
        parts.push(http.hostname);
      }
      if (http.url) {
        parts.push(http.url);
      }
      return parts.join(" ");
    }
    case "smb": {
      const smb = event._source.smb;
      return `${smb?.command} - ${smb?.status} (${smb?.dialect})`;
    }
    case "ssh": {
      const ssh = event._source.ssh;
      return `${ssh?.client?.software_version || "Unknown"}/${
        ssh?.client?.proto_version || "Unknown"
      } => ${ssh?.server?.software_version || "Unknown"}/${
        ssh?.server?.proto_version || "Unknown"
      }`;
    }
    case "stats": {
      const stats = event._source.stats!;
      let parts = [];
      if (stats.decoder.pkts !== undefined) {
        parts.push(`Packets=${stats.decoder.pkts}`);
      }
      if (stats.decoder.bytes !== undefined) {
        parts.push(`Bytes=${stats.decoder.bytes}`);
      }
      if (stats.capture?.kernel_drops !== undefined) {
        parts.push(`Drops=${stats.capture.kernel_drops}`);
      }
      if (stats.uptime !== undefined) {
        parts.push(`Uptime: ${get_duration(stats.uptime).humanize()}`);
      }
      return parts.join(" ");
    }
    case "quic": {
      let quic = event._source.quic;
      let parts = [];
      if (quic.version) {
        parts.push(`Version ${quic.version}`);
      }
      if (quic.sni) {
        parts.push(`SNI ${quic.sni}`);
      }
      return parts.join("; ");
    }
    case "sip": {
      let sip = event._source.sip;
      if (sip.request_line) {
        return `REQUEST: ${sip.request_line}`;
      } else if (sip.response_line) {
        return `RESPONSE: ${sip.response_line}`;
      } else {
        return `${JSON.stringify(sip)}`;
      }
    }
    default: {
      const event_type = event._source.event_type;
      if (event_type && event._source[event_type]) {
        return JSON.stringify(event._source[event_type]);
      }
      return JSON.stringify(event._source);
    }
  }
}

export function formatAddressWithPort(
  addr: string,
  port: undefined | number
): string {
  if (port) {
    return `${formatAddress(addr)}:[${port}]`;
  } else {
    return formatAddress(addr);
  }
}

export function formatAddress(addr: string) {
  return addr.replace(/(0000\:)+/, ":");
}
