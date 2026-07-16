// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Event } from "./types";
import { get_duration } from "./datetime";

const FTP_SENSITIVE_COMMANDS = new Set(["PASS", "ACCT", "ADAT"]);

const RDP_PROTOCOLS: Record<string, string> = {
  rdp: "RDP Security",
  ssl: "TLS",
  hybrid: "CredSSP",
  rdstls: "RDSTLS",
  hybrid_ex: "CredSSP (HYBRID_EX)",
};

const RDP_CAPABILITIES: Record<string, string> = {
  extended_client_data: "extended client data",
  dynvc_gfx: "dynamic graphics channels",
  restricted_admin: "Restricted Admin",
  redirected_authentication: "redirected authentication",
};

const SNMP_PDU_TYPES: Record<string, string> = {
  get_request: "GET request",
  get_next_request: "GET-NEXT request",
  get_bulk_request: "GET-BULK request",
  set_request: "SET request",
  response: "Response",
  trap_v1: "Trap",
  trap_v2: "Trap v2",
  inform_request: "Inform request",
  report: "Report",
  encrypted: "Encrypted PDU",
};

const MQTT_PROTOCOL_VERSIONS: Record<number, string> = {
  3: "3.1",
  4: "3.1.1",
  5: "5.0",
};

const MQTT_MESSAGE_TYPES = [
  "connect",
  "connack",
  "publish",
  "puback",
  "pubrec",
  "pubrel",
  "pubcomp",
  "subscribe",
  "suback",
  "unsubscribe",
  "unsuback",
  "pingreq",
  "pingresp",
  "auth",
  "disconnect",
];

const IKE_EXCHANGE_TYPES: Record<number, Record<number, string>> = {
  1: {
    0: "None",
    1: "Base",
    2: "Main Mode",
    3: "Authentication Only",
    4: "Aggressive",
    5: "Informational",
    6: "Transaction",
    32: "Quick Mode",
    33: "New Group Mode",
  },
  2: {
    34: "IKE_SA_INIT",
    35: "IKE_AUTH",
    36: "CREATE_CHILD_SA",
    37: "INFORMATIONAL",
    38: "IKE_SESSION_RESUME",
    39: "GSA_AUTH",
    40: "GSA_REGISTRATION",
    41: "GSA_REKEY",
    43: "IKE_INTERMEDIATE",
    44: "IKE_FOLLOWUP_KE",
  },
};

function formatMqttTopics(topics: any[] | undefined): string | undefined {
  if (!topics || topics.length === 0) {
    return undefined;
  }

  const topic = typeof topics[0] === "string" ? topics[0] : topics[0].topic;
  return topics.length > 1 ? `${topic} (+${topics.length - 1} more)` : topic;
}

function formatSmtpValue(value: unknown, maxLength = 100): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }

  const text = value.replace(/\s+/g, " ").trim();
  if (!text) {
    return undefined;
  }
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

function formatSmtpRecipients(value: unknown): string | undefined {
  const values = Array.isArray(value) ? value : [value];
  const recipients = values
    .map((recipient) => formatSmtpValue(recipient))
    .filter((recipient): recipient is string => recipient !== undefined);

  if (recipients.length === 0) {
    return undefined;
  }
  return recipients.length > 1
    ? `${recipients[0]} (+${recipients.length - 1} more)`
    : recipients[0];
}

function formatProtocolText(
  value: unknown,
  maxLength = 120,
): string | undefined {
  if (
    typeof value !== "string" ||
    /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f\ufffd]/.test(value)
  ) {
    return undefined;
  }

  const text = value.replace(/\s+/g, " ").trim();
  if (!text) {
    return undefined;
  }
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

function formatFtpReply(value: unknown): string | undefined {
  const values = Array.isArray(value) ? value : [value];
  const replies = values
    .map((reply) => formatProtocolText(reply))
    .filter((reply): reply is string => reply !== undefined);

  if (replies.length === 0) {
    return undefined;
  }
  return replies.length > 1
    ? `${replies[0]} (+${replies.length - 1} more)`
    : replies[0];
}

function formatFtpCodes(value: unknown): string | undefined {
  const values = Array.isArray(value) ? value : [value];
  const codes = values
    .filter((code) => typeof code === "string" || typeof code === "number")
    .map(String);
  return codes.length > 0 ? codes.join("/") : undefined;
}

export function formatEventDescription(event: Event): string {
  try {
    const source = event._source;
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
          event._source.src_port,
        );
        const dest = formatAddressWithPort(
          event._source.dest_ip,
          event._source.dest_port,
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
      case "ftp": {
        const ftp = event._source.ftp || {};
        const command = formatProtocolText(ftp.command)?.toUpperCase();
        const code = formatFtpCodes(ftp.completion_code);
        const reply = formatFtpReply(ftp.reply);
        let description;

        if (command) {
          const commandData = FTP_SENSITIVE_COMMANDS.has(command)
            ? ftp.command_data
              ? "[data hidden]"
              : undefined
            : formatProtocolText(ftp.command_data);
          description = [command, commandData].filter(Boolean).join(" ");
          if (ftp.command_truncated) {
            description += "…";
          }
        } else if (code || reply) {
          return [code, reply].filter(Boolean).join(" ");
        } else if (ftp.reply) {
          return "FTP response (non-text payload)";
        } else {
          return "FTP transaction";
        }

        const response = [code, reply].filter(Boolean).join(" ");
        return response ? `${description} — ${response}` : description;
      }
      case "ftp_data": {
        const ftpData = event._source.ftp_data || {};
        const command = formatProtocolText(ftpData.command)?.toUpperCase();
        const filename = formatProtocolText(ftpData.filename);
        return (
          [command, filename].filter(Boolean).join(" ") || "FTP data transfer"
        );
      }
      case "flow": {
        const packets =
          event._source.flow?.pkts_toclient! +
            event._source.flow?.pkts_toserver! || "N/A";
        const bytes =
          event._source.flow?.bytes_toclient! +
            event._source.flow?.bytes_toserver! || "N/A";
        const source = formatAddressWithPort(
          event._source.src_ip,
          event._source.src_port,
        );
        const dest = formatAddressWithPort(
          event._source.dest_ip,
          event._source.dest_port,
        );
        let age =
          event._source.flow?.age != undefined ? event._source.flow.age : "N/A";
        let parts = [
          event._source.proto,
          `${source} => ${dest}`,
          `Age=${age}`,
          `Packets=${packets}`,
          `Bytes=${bytes}`,
        ];
        return parts.join(" ");
      }
      case "mqtt": {
        const mqtt = event._source.mqtt;

        if (mqtt.publish) {
          const publish = mqtt.publish;
          const details = [];

          if (publish.qos !== undefined) {
            details.push(`QoS ${publish.qos}`);
          }
          if (publish.message_id !== undefined) {
            details.push(`msg ${publish.message_id}`);
          }
          if (publish.retain) {
            details.push("retained");
          }
          if (publish.dup) {
            details.push("duplicate");
          }
          for (const acknowledgement of [
            "puback",
            "pubrec",
            "pubrel",
            "pubcomp",
          ]) {
            if (mqtt[acknowledgement]) {
              details.push(acknowledgement.toUpperCase());
            }
          }

          const description = ["PUBLISH", publish.topic]
            .filter(Boolean)
            .join(" ");
          return details.length > 0
            ? `${description} — ${details.join(", ")}`
            : description;
        }

        if (mqtt.connect) {
          const connect = mqtt.connect;
          const protocol = connect.protocol_string || "MQTT";
          const version =
            MQTT_PROTOCOL_VERSIONS[connect.protocol_version] ||
            connect.protocol_version;
          const description = ["CONNECT", protocol, version]
            .filter((part) => part !== undefined)
            .join(" ");
          const details = [];

          if (connect.client_id) {
            details.push(`client ${connect.client_id}`);
          }
          if (mqtt.connack?.return_code === 0) {
            details.push("accepted");
          } else if (mqtt.connack?.return_code !== undefined) {
            details.push(`CONNACK code ${mqtt.connack.return_code}`);
          }

          return details.length > 0
            ? `${description} — ${details.join(", ")}`
            : description;
        }

        if (mqtt.subscribe) {
          const subscribe = mqtt.subscribe;
          const description = ["SUBSCRIBE", formatMqttTopics(subscribe.topics)]
            .filter(Boolean)
            .join(" ");
          const details = [];
          const requestedQos = subscribe.topics
            ?.map((topic: any) => topic.qos)
            .filter((qos: any) => qos !== undefined);

          if (requestedQos?.length) {
            details.push(`requested QoS ${requestedQos.join(", ")}`);
          }
          if (mqtt.suback?.qos_granted?.length) {
            const grantedQos = mqtt.suback.qos_granted;
            if (grantedQos.every((qos: number) => qos >= 0 && qos <= 2)) {
              details.push(`granted QoS ${grantedQos.join(", ")}`);
            } else {
              details.push(`SUBACK codes ${grantedQos.join(", ")}`);
            }
          }
          return details.length > 0
            ? `${description} — ${details.join(", ")}`
            : description;
        }

        if (mqtt.unsubscribe) {
          const description = [
            "UNSUBSCRIBE",
            formatMqttTopics(mqtt.unsubscribe.topics),
          ]
            .filter(Boolean)
            .join(" ");
          return mqtt.unsuback ? `${description} — UNSUBACK` : description;
        }

        const messageTypes = MQTT_MESSAGE_TYPES.filter((type) => mqtt[type]);
        if (messageTypes.length === 0) {
          messageTypes.push(
            ...Object.keys(mqtt).filter(
              (type) => type !== "type" && typeof mqtt[type] === "object",
            ),
          );
        }
        const message = mqtt[messageTypes[0]];
        const details = [];

        if (message?.message_id !== undefined) {
          details.push(`msg ${message.message_id}`);
        }
        if (message?.reason_code !== undefined) {
          details.push(`reason ${message.reason_code}`);
        }
        if (message?.truncated) {
          details.push("truncated");
        }

        const description =
          messageTypes.map((type) => type.toUpperCase()).join("/") || "MQTT";
        return details.length > 0
          ? `${description} — ${details.join(", ")}`
          : description;
      }
      case "netflow": {
        const netflow = event._source.netflow!;
        formatAddress(event._source.src_ip);
        const source = formatAddressWithPort(
          event._source.src_ip,
          event._source.src_port,
        );
        const dest = formatAddressWithPort(
          event._source.dest_ip,
          event._source.dest_port,
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
      case "tftp": {
        const tftp = event._source.tftp || {};
        const packet = formatProtocolText(tftp.packet)?.toUpperCase();
        const filename = formatProtocolText(tftp.file);
        const mode = formatProtocolText(tftp.mode);
        const description = [packet, filename].filter(Boolean).join(" ");
        return mode
          ? `${description || "TFTP transfer"} — ${mode} mode`
          : description || "TFTP transfer";
      }
      case "tls": {
        const tls = source.tls!;
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
      case "ike": {
        const ike = event._source.ike;
        const version = ike.version_minor
          ? `IKEv${ike.version_major}.${ike.version_minor}`
          : `IKEv${ike.version_major}`;
        const exchange =
          ike.exchange_type_verbose ||
          IKE_EXCHANGE_TYPES[ike.version_major]?.[ike.exchange_type] ||
          `exchange ${ike.exchange_type}`;
        const details = [];

        if (ike.role) {
          details.push(ike.role);
        }
        if (ike.message_id !== undefined) {
          details.push(`msg ${ike.message_id}`);
        }
        if (ike.ikev2?.errors) {
          const suffix = ike.ikev2.errors === 1 ? "" : "s";
          details.push(`${ike.ikev2.errors} error${suffix}`);
        }

        const description = [version, exchange].join(" ");
        return details.length > 0
          ? `${description} — ${details.join(", ")}`
          : description;
      }
      case "smb": {
        const smb = event._source.smb;
        return `${smb?.command} - ${smb?.status} (${smb?.dialect})`;
      }
      case "snmp": {
        const snmp = event._source.snmp || {};
        const version =
          snmp.version === 2
            ? "SNMPv2c"
            : snmp.version !== undefined
              ? `SNMPv${snmp.version}`
              : "SNMP";
        const pduType = snmp.pdu_type
          ? SNMP_PDU_TYPES[snmp.pdu_type] || snmp.pdu_type.replace(/_/g, " ")
          : undefined;
        const vars = snmp.vars || [];
        const variable = formatProtocolText(vars[0]);
        const variableSuffix =
          vars.length > 1 ? ` (+${vars.length - 1} more)` : "";
        const description = [version, pduType].filter(Boolean).join(" ");
        return variable
          ? `${description} — ${variable}${variableSuffix}`
          : description;
      }
      case "smtp": {
        const smtp = event._source.smtp || {};
        const sender = formatSmtpValue(smtp.mail_from);
        const recipients = formatSmtpRecipients(smtp.rcpt_to);
        const helo = formatSmtpValue(smtp.helo);
        const subject = formatSmtpValue(event._source.email?.subject);
        let description;

        if (sender && recipients) {
          description = `MAIL FROM ${sender} → RCPT TO ${recipients}`;
        } else if (sender) {
          description = `MAIL FROM ${sender}`;
        } else if (recipients) {
          description = `RCPT TO ${recipients}`;
        } else if (helo) {
          return `HELO ${helo}`;
        } else {
          return "SMTP transaction";
        }

        const details = [];
        if (subject) {
          details.push(`subject “${subject}”`);
        }
        if (helo) {
          details.push(`HELO ${helo}`);
        }
        return details.length > 0
          ? `${description} — ${details.join(", ")}`
          : description;
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
      case "rdp": {
        const rdp = event._source.rdp || {};

        if (rdp.event_type === "initial_request") {
          const cookie = formatProtocolText(rdp.cookie);
          return cookie
            ? `Initial request — cookie ${cookie}`
            : "Initial request";
        }

        if (rdp.event_type === "initial_response") {
          const protocol = rdp.protocol
            ? RDP_PROTOCOLS[rdp.protocol] || rdp.protocol.toUpperCase()
            : undefined;
          const capabilities = rdp.server_supports
            ?.map(
              (capability: string) =>
                RDP_CAPABILITIES[capability] || capability.replace(/_/g, " "),
            )
            .join(", ");
          const details = [
            protocol ? `protocol ${protocol}` : undefined,
            capabilities ? `supports ${capabilities}` : undefined,
          ].filter(Boolean);
          return details.length > 0
            ? `Initial response — ${details.join("; ")}`
            : "Initial response";
        }

        if (rdp.event_type === "tls_handshake") {
          const serials = rdp.x509_serials || [];
          const certificate = serials[0]
            ? `certificate serial ${serials[0]}`
            : undefined;
          const suffix =
            serials.length > 1 ? ` (+${serials.length - 1} more)` : "";
          return certificate
            ? `TLS handshake — ${certificate}${suffix}`
            : "TLS handshake";
        }

        const eventType = formatProtocolText(rdp.event_type)?.replace(
          /_/g,
          " ",
        );
        return eventType ? `RDP ${eventType}` : "RDP transaction";
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
      case "llmnr": {
        const llmnr = event._source.llmnr;
        const parts = [llmnr.type.toUpperCase()];
        const record =
          llmnr.type === "request"
            ? llmnr.queries?.[0]
            : (llmnr.answers?.[0] ?? llmnr.queries?.[0]);

        if (record?.rrtype) {
          parts.push(record.rrtype.toUpperCase());
        }
        if (record?.rrname) {
          parts.push(record.rrname);
        }
        if (llmnr.type === "response" && record?.rdata) {
          parts.push("→", record.rdata);
        }
        return parts.join(" ");
      }
      case "mdns": {
        let mdns = event._source.mdns;
        let parts = [mdns.type.toUpperCase()];
        if (mdns.type == "request") {
          parts.push(mdns.queries[0].rrname);
        } else {
          parts.push(mdns.answers[0].rrname);
        }
        return parts.join(" ");
      }
      default: {
        const event_type = event._source.event_type;
        if (event_type && event._source[event_type]) {
          return JSON.stringify(event._source[event_type]);
        }
        return JSON.stringify(event._source);
      }
    }
  } catch (e) {
    console.log(`Failed to format event description: ${e}`);
    return "Failed to render event";
  }
}

export function formatAddressWithPort(
  addr: string,
  port: undefined | number,
): string {
  if (port) {
    return `${formatAddress(addr)}:[${port}]`;
  } else {
    return formatAddress(addr);
  }
}

export function formatAddress(addr: string) {
  if (!addr) {
    return "";
  }
  return addr.replace(/(0000\:)+/, ":");
}
