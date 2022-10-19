// Copyright (C) 2014-2021 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

import { Pipe, PipeTransform } from "@angular/core";
import { EveboxFormatIpAddressPipe } from "./format-ipaddress.pipe";

@Pipe({
    name: "eveboxEventDescriptionPrinter",
})
export class EveBoxEventDescriptionPrinterPipe implements PipeTransform {
    constructor(private ipFormatter: EveboxFormatIpAddressPipe) {}

    formatDnsRequest(event: any) {
        let dns = event.dns;
        return `QUERY ${dns.rrtype} ${dns.rrname}`;
    }

    formatDnsResponse(event: any) {
        let dns: any = event.dns;
        switch (dns.version) {
            case 2:
                switch (dns.rcode) {
                    case "NXDOMAIN":
                    case "SERVFAIL":
                        return `ANSWER: ${dns.rcode} ${dns.rrname}`;
                    case "NOERROR":
                        if (!dns.answers) {
                            return `ANSWER: ${dns.rrname} [no answers]`;
                        }
                        let last = dns.answers[dns.answers.length - 1];
                        return `ANSWER: ${dns.rrname} ${last.rrtype} ${last.rdata}`;
                    default:
                        return `OOPS: We don't know how to format this DNS response.`;
                }
            default:
                switch (dns.rcode) {
                    case "NXDOMAIN":
                        return `ANSWER: NXDOMAIN for ${dns.rrname}`;
                    default:
                        return `ANSWER for ${dns.rrname}: ${dns.rrtype} ${
                            dns.rdata || ""
                        }`;
                }
        }
    }

    formatDns(eve: any) {
        if (eve.dns.type == "answer") {
            return this.formatDnsResponse(eve);
        } else if (eve.dns.type == "query") {
            return this.formatDnsRequest(eve);
        } else {
            return `UNSUPPORTED DNS TYPE "${eve.dns.type}"`;
        }
    }

    formatSmb(eve: any) {
        let smb = eve.smb;
        let ntlmssp = smb.ntlmssp || {};
        let msg = `${smb.command}`;
        if (smb.dialect) {
            msg = `${msg}, Dialect: ${smb.dialect}`;
        }
        if (smb.status) {
            msg = `${msg}, Status: ${smb.status}`;
        }
        if (smb.filename) {
            msg = `${msg}, Filename: ${smb.filename}`;
        }
        if (ntlmssp.domain) {
            msg = `${msg}, Host: ${ntlmssp.domain}`;
        }
        if (ntlmssp.host) {
            msg = `${msg}, Host: ${ntlmssp.host}`;
        }

        return msg;
    }

    transform(event: any): string {
        if (!event._source.event_type) {
            return "[Error: This does not look like an event]";
        }

        const eve = event._source;

        const srcAddr = this.ipFormatter.transform(eve.src_ip);
        const destAddr = this.ipFormatter.transform(eve.dest_ip);

        switch (event._source.event_type) {
            case "alert": {
                const alert = event._source.alert;
                if (alert.signature) {
                    return alert.signature;
                } else {
                    return (
                        `ALERT: [${alert.gid}:${alert.signature_id}:${alert.rev}]` +
                        ` (${alert.category})`
                    );
                }
            }
            case "http": {
                const http = event._source.http;
                return `${http.http_method} - ${http.hostname} - ${http.url}`;
            }
            case "ssh": {
                const ssh = eve.ssh;
                return `${ssh.client.software_version} -> ${ssh.server.software_version}`;
            }
            case "tls": {
                return `${eve.tls.version} - ${eve.tls.sni || "[no sni]"} - ${
                    eve.tls.subject || "[no subject]"
                }`;
            }
            case "flow": {
                const flow = eve.flow;
                let sport = "";
                let dport = "";
                switch (eve.proto.toLowerCase()) {
                    case "udp":
                    case "tcp":
                        sport = `:${eve.src_port}`;
                        dport = `:${eve.dest_port}`;
                        break;
                }
                return (
                    `${eve.proto} ${srcAddr}${sport} -> ${destAddr}${dport}` +
                    `; Age: ${flow.age}` +
                    `; Bytes: ${flow.bytes_toserver + flow.bytes_toclient}` +
                    `; Packets: ${flow.pkts_toserver + flow.pkts_toclient}`
                );
            }
            case "netflow": {
                const netflow = eve.netflow;
                let sport = "";
                let dport = "";
                switch (eve.proto.toLowerCase()) {
                    case "udp":
                    case "tcp":
                        sport = `:${eve.src_port}`;
                        dport = `:${eve.dest_port}`;
                        break;
                }
                return (
                    `${eve.proto} ${srcAddr}${sport} -> ${destAddr}${dport}` +
                    `; Age: ${netflow.age}` +
                    `; Bytes: ${netflow.bytes}` +
                    `; Packets: ${netflow.pkts}`
                );
            }
            case "dns": {
                return this.formatDns(eve);
            }
            case "drop":
                const drop: any = eve.drop;
                let srcPort = "";
                let dstPort = "";
                if (eve.src_port) {
                    srcPort = `:${eve.src_port}`;
                }
                if (eve.dest_port) {
                    dstPort = `:${eve.dest_port}`;
                }

                let flags: string[] = [];
                if (drop.syn) {
                    flags.push("SYN");
                }
                if (drop.ack) {
                    flags.push("ACK");
                }
                if (drop.psh) {
                    flags.push("PSH");
                }
                if (drop.rst) {
                    flags.push("RST");
                }
                if (drop.urg) {
                    flags.push("URG");
                }
                if (drop.fin) {
                    flags.push("FIN");
                }
                const flagInfo = flags.join(",");

                return `${eve.proto} - ${eve.src_ip}${srcPort} -> ${eve.dest_ip}${dstPort} [${flagInfo}]`;
            case "fileinfo":
                const extra: string[] = [];

                if (eve.http && eve.http.hostname) {
                    extra.push(`Hostname: ${eve.http.hostname}`);
                }
                if (eve.http && eve.http.http_content_type) {
                    extra.push(`Content-Type: ${eve.http.http_content_type}`);
                }

                const extraInfo = "- " + extra.join("; ");

                return `${eve.fileinfo.filename} ${extraInfo}`;
            case "smb":
                return this.formatSmb(eve);
            case "dhcp":
                return this.formatDhcp(event);
            case "anomaly": {
                const anom = eve.anomaly;
                if (anom.event) {
                    return `${anom.type.toUpperCase()}: ${anom.event}`;
                } else if (anom.code) {
                    return `${anom.type.toUpperCase()}: ${anom.code}`;
                }
            }
            case "stats": {
                const captureStats = event._source.stats.capture;
                return `Packets: ${captureStats.kernel_packets}, Drops: ${captureStats.kernel_drops}`;
            }
            default:
                break;
        }
        return JSON.stringify(event._source[event._source.event_type]);
    }

    formatDhcp(event: any): string {
        const dhcp = event._source["dhcp"];
        let client_mac = dhcp.client_mac;

        let with_fields = [];
        if (dhcp.assigned_ip && dhcp.assigned_ip != "0.0.0.0") {
            with_fields.push(`assigned-ip=${dhcp.assigned_ip}`);
        }
        if (dhcp.client_ip && dhcp.client_ip != "0.0.0.0") {
            with_fields.push(`client-ip=${dhcp.client_ip}`);
        }
        if (dhcp.hostname) {
            with_fields.push(`hostname=${dhcp.hostname}`);
        }

        let with_string = "";
        if (with_fields.length > 0) {
            with_string = "with " + with_fields.join(", ");
        }

        if (dhcp.dhcp_type == "ack") {
            return `Ack to ${client_mac} ${with_string}`;
        } else if (dhcp.dhcp_type == "request") {
            return `Request from ${client_mac} ${with_string}`;
        } else if (dhcp.dhcp_type == "offer") {
            return `Offer to ${client_mac} ${with_string}`;
        } else if (dhcp.dhcp_type == "discover") {
            return `Discover from ${client_mac} ${with_string}`;
        } else if (dhcp.dhcp_type == "inform") {
            return `Inform from ${client_mac} ${with_string}`;
        } else if (dhcp.dhcp_type == "release") {
            return `Release from ${client_mac} ${with_string}`;
        } else {
            console.log("Unknown DHCP type: " + dhcp.dhcp_type);
        }
        return JSON.stringify(event._source["dhcp"]);
    }
}
