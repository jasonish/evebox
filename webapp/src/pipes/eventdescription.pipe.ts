/* Copyright (c) 2014-2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

import {Pipe, PipeTransform} from "@angular/core";
import {EveboxFormatIpAddressPipe} from "./format-ipaddress.pipe";

@Pipe({
    name: "eveboxEventDescriptionPrinter"
})
export class EveBoxEventDescriptionPrinterPipe implements PipeTransform {

    constructor(private ipFormatter:EveboxFormatIpAddressPipe) {
    }

    transform(value:any, args:any):any {

        let event = value;

        if (!event._source.event_type) {
            return "<Error: This does not look like an event.>";
        }

        let eve = event._source;

        let srcAddr = this.ipFormatter.transform(eve.src_ip);
        let destAddr = this.ipFormatter.transform(eve.dest_ip);

        switch (event._source.event_type) {
            case "alert": {
                let alert = event._source.alert;
                if (alert.signature) {
                    return alert.signature;
                }
                else {
                    return `ALERT: [${alert.gid}:${alert.signature_id}:${alert.rev}]`
                        + ` (${alert.category})`;
                }
            }
            case "http": {
                let http = event._source.http;
                return `${http.http_method} - ${http.hostname} - ${http.url}`;
            }
            case "ssh": {
                let ssh = eve.ssh;
                return `${ssh.client.software_version} -> ${ssh.server.software_version}`;
            }
            case "tls": {
                return `${eve.tls.version} - ${eve.tls.subject}`;
            }
            case "flow": {
                let flow = eve.flow;
                let sport = "";
                let dport = "";
                switch (eve.proto.toLowerCase()) {
                    case "udp":
                    case "tcp":
                        sport = `:${eve.src_port}`;
                        dport = `:${eve.dest_port}`;
                        break;
                }
                return `${eve.proto} ${srcAddr}${sport} -> ${destAddr}${dport}`
                    + `; Age: ${flow.age}`
                    + `; Bytes: ${flow.bytes_toserver + flow.bytes_toclient}`
                    + `; Packets: ${flow.pkts_toserver + flow.pkts_toclient}`;
            }
            case "netflow": {
                let netflow = eve.netflow;
                let sport = "";
                let dport = "";
                switch (eve.proto.toLowerCase()) {
                    case "udp":
                    case "tcp":
                        sport = `:${eve.src_port}`;
                        dport = `:${eve.dest_port}`;
                        break;
                }
                return `${eve.proto} ${srcAddr}${sport} -> ${destAddr}${dport}`
                    + `; Age: ${netflow.age}`
                    + `; Bytes: ${netflow.bytes}`
                    + `; Packets: ${netflow.pkts}`;
            }
            case "dns": {
                let dns = eve.dns;
                let desc = "";
                switch (dns.type) {
                    case "query":
                        desc += `QUERY ${dns.rrtype} ${dns.rrname}`;
                        break;
                    case "answer":
                        switch (dns.rcode) {
                            case "NXDOMAIN":
                                desc += `ANSWER: NXDOMAIN for ${dns.rrname}`;
                                break;
                            default:
                                desc += `ANSWER for ${dns.rrname}: ${dns.rrtype} ${dns.rdata || ""}`;
                                break;
                        }
                        break;
                }
                return `${desc}`
            }
            default:
                return JSON.stringify(event._source[event._source.event_type]);
        }
    }
}
