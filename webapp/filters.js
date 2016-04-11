/* Copyright (c) 2014 Jason Ish
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

import angular from "angular";
import jQuery from "jquery";
import moment from "moment";

function formatIpAddress(addr) {
    if (addr === undefined) {
        return "";
    }
    addr = addr.replace(/0000/g, "");
    while (addr.indexOf(":0:") > -1) {
        addr = addr.replace(/:0:/g, "::");
    }
    addr = addr.replace(/:::+/g, "::");
    while (addr != (addr = addr.replace(/:0+/g, ":")))
        ;
    return addr;
}

angular.module("app").filter("formatIpAddress", function () {
    return formatIpAddress;
});

angular.module("app").filter("formatTimestamp", function () {
    return function (timestamp, format) {
        return moment(timestamp).format("YYYY-MM-DD HH:mm:ss");
    }
});

function eventSeverityToBootstrapClass(event, prefix) {

    if (prefix === undefined) {
        prefix = "";
    }

    let severity = 0;

    if (Number(event) === event) {
        severity = event;
    }
    else {
        switch (event._source.event_type) {
            case "alert":
                severity = event._source.alert.severity;
                break;
            default:
                break;
        }
    }

    switch (severity) {
        case 1:
            return `${prefix}danger`;
        case 2:
            return `${prefix}warning`;
        case 3:
            return `${prefix}info`;
        default:
            return `${prefix}success`;
    }

}

angular.module("app").filter("eventSeverityToBootstrapClass", function () {
    return eventSeverityToBootstrapClass;
});

function formatEventDescription() {
    return function (event) {

        if (!event._source.event_type) {
            return "<Error: This does not look like an event.>";
        }

        let eve = event._source;

        switch (event._source.event_type) {
            case "alert":
                return event._source.alert.signature;
            case "http":
            {
                let http = event._source.http;
                return `${http.http_method} - ${http.hostname} - ${http.url}`;
            }
            case "ssh": {
                let ssh = eve.ssh;
                return `${ssh.client.software_version} -> ${ssh.server.software_version}`;
            }
            case "tls": {
                let tls = eve.tls;
                let cn = /CN=(.*)/.exec(tls.subject)[1];
                let issuer = /CN=(.*)/.exec(tls.issuerdn)[1];
                return `${tls.version}: ${cn} (Issuer: ${issuer})`;
            }
            case "flow":
            {
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
                return `${eve.proto} ${eve.src_ip}${sport} -> ${eve.dest_ip}${dport}`
                    + `; Age: ${flow.age}`
                    + `; Bytes: ${flow.bytes_toserver + flow.bytes_toclient}`
                    + `; Packets: ${flow.pkts_toserver + flow.pkts_toclient}`;
            }
            case "netflow":
            {
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
                return `${eve.proto} ${eve.src_ip}${sport} -> ${eve.dest_ip}${dport}`
                    + `; Age: ${netflow.age}`
                    + `; Bytes: ${netflow.bytes}`
                    + `; Packets: ${netflow.pkts}`;
                break;
            }
            case "dns":
            {
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

angular.module("app").filter("formatEventDescription", formatEventDescription);

angular.module("app").filter("base64Decode", function () {
    return base64Decode
});

function base64Decode(input) {
    return atob(input);
}

angular.module("app").filter("base64ToFormattedHex", function () {
    return base64ToFormattedHex;
});

function base64ToFormattedHex(input) {
    for (var i = 0, bin = atob(
        input.replace(/[ \r\n]+$/, "")), hex = []; i < bin.length; ++i) {
        var tmp = bin.charCodeAt(i).toString(16);
        if (tmp.length === 1)
            tmp = "0" + tmp;
        hex[hex.length] = tmp;
    }
    let output = "";
    for (let i = 0; i < hex.length; i++) {
        if (i > 0 && i % 16 == 0) {
            output += "\n"
        }
        output += hex[i] + " ";
    }
    return output;

}

angular.module("app").filter("genericEvePrettyPrinter", function () {

    let map = {
        "hostname": "Hostname",
        "url": "URL",
        "http_user_agent": "User Agent",
        "http_content_type": "ContentType",
        "http_method": "Method",
        "protocol": "Protocol",
        "status": "Status",
        "length": "Content Length",
    };

    return function (value) {
        let pretty = map[value];
        return pretty ? pretty : value;
    }
});