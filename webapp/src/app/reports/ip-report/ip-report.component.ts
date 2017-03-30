/* Copyright (c) 2016 Jason Ish
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
import {Component, OnInit, OnDestroy} from '@angular/core';
import {ActivatedRoute} from '@angular/router';
import {EveboxSubscriptionService} from '../../subscription.service';
import {ElasticSearchService} from '../../elasticsearch.service';
import {TopNavService} from '../../topnav.service';
import {ReportsService} from '../reports.service';
import {AppService, AppEvent, AppEventCode} from '../../app.service';
import {loadingAnimation} from '../../animations';
import * as moment from 'moment';
import {humanizeFileSize} from '../../humanize.service';
import {ApiService} from '../../api.service';

function termQuery(type: string, field: string, value: string) {
    let term = {};
    term[type] = {};
    term[type][field] = value;
    return term;
}

@Component({
    templateUrl: './ip-report.component.html',
    animations: [
        loadingAnimation,
    ]
})
export class IpReportComponent implements OnInit, OnDestroy {

    ip: string;

    loading = 0;

    alertsOverTime: any[];

    flow: any = {

        ready: false,

        sourceFlowCount: 0,
        destFlowCount: 0,
        bytesToIp: 0,
        bytesFromIp: 0,
        packetsToIp: 0,
        packetsFromIp: 0,
    };

    // DNS hostname lookups returning this IP.
    dnsHostnamesForAddress: any[];

    // Top requested hostnames.
    dnsRequestedHostnames: any[];

    userAgents: any[];

    topHttpHostnames: any[];

    tlsSni: any[];

    topTlsSniRequests: any[];

    tlsClientVersions: any[];

    tlsServerVersions: any[];

    topTlsSubjectRequests: any[];

    topDestinationHttpHostnames: any[];

    topSignatures: any[];

    ssh: any = {
        sshInboundClientVersions: [],
        sshOutboundClientVersions: [],
        sshOutboundServerVersions: [],
        sshInboundServerVersions: [],
        sshOutboundClientProtoVersions: [],
        sshOutboundServerProtoVersions: [],
        sshInboundClientProtoVersions: [],
        sshInboundServerProtoVersions: [],
    };

    sensors: Set<string> = new Set<string>();

    // Empty string defaults to all sensors.
    sensorFilter = '';

    queryString = '';

    constructor(private route: ActivatedRoute,
                private elasticsearch: ElasticSearchService,
                private appService: AppService,
                private topNavService: TopNavService,
                private reportsService: ReportsService,
                private api: ApiService,
                private ss: EveboxSubscriptionService) {
    }

    ngOnInit() {
        this.ss.subscribe(this, this.route.params, (params: any) => {
            this.ip = params.ip;
            this.queryString = params.q;
            this.refresh();
            this.buildRelated(this.ip);
        });

        this.ss.subscribe(this, this.appService, (event: AppEvent) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });
    }

    relatedAddresses: any[] = [];

    buildRelated(ip: any) {

        this.relatedAddresses = [];

        let sep = '.';

        if (ip.indexOf(':') > -1) {
            // Looks like IPv6.
            sep = ':';
        }

        let parts = ip.split(sep).filter((part: any) => {
            return part != '';
        });

        if (sep == ':') {
            while (parts.length > 1) {
                parts.splice(parts.length - 1, 1);
                this.relatedAddresses.push({
                    value: parts.join(sep) + sep,
                    name: parts.join(sep) + sep,
                });
            }
        }
        else {
            // The above generic loop could be used for IPv4 as well, but
            // this gives better use about with CIDR notation.
            if (parts.length > 3) {
                this.relatedAddresses.push({
                    value: `${parts[0]}.${parts[1]}.${parts[2]}.`,
                    name: `${parts[0]}.${parts[1]}.${parts[2]}/24`
                });
            }

            if (parts.length > 2) {
                this.relatedAddresses.push({
                    value: `${parts[0]}.${parts[1]}.`,
                    name: `${parts[0]}.${parts[1]}/16`
                });
            }

            if (parts.length > 1) {
                this.relatedAddresses.push({
                    value: `${parts[0]}.`,
                    name: `${parts[0]}/8`
                });
            }

        }
    }

    ngOnDestroy() {
        this.ss.unsubscribe(this);
    }

    keywordTermQuery(keyword: string, value: any): any {
        return this.elasticsearch.keywordTerm(keyword, value);
    }

    asKeyword(keyword: string): string {
        return this.elasticsearch.asKeyword(keyword);
    }

    termQuery(type: string, field: string, value: string) {
        let term = {};
        term[type] = {};
        term[type][field] = value;
        return term;
    }

    queryDnsHostnamesForAddress(range: any, now: any) {

        this.loading++;

        let ipTermType = 'term';

        if (this.ip[this.ip.length - 1] == '.') {
            ipTermType = 'prefix';
        }

        let query = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: 'event_type'}},
                        {term: {'event_type': 'dns'}},
                        this.keywordTermQuery('dns.type', 'answer'),
                        termQuery(ipTermType, 'dns.rdata', this.ip),
                    ]
                }
            },
            size: 0,
            aggs: {
                uniqueHostnames: {
                    terms: {
                        field: this.asKeyword('dns.rrname'),
                        size: 100,
                    }
                }
            }
        };

        if (this.sensorFilter != '') {
            this.elasticsearch.addSensorNameFilter(query, this.sensorFilter);
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response: any) => {

            this.dnsHostnamesForAddress = response.aggregations.uniqueHostnames.buckets.map((bucket: any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                };
            });

            this.loading--;

        });
    }

    refresh() {

        let range = this.topNavService.getTimeRangeAsSeconds();
        let now = moment();

        this.queryDnsHostnamesForAddress(range, now);

        this.loading++;

        let ipTermType = 'term';

        if (this.ip[this.ip.length - 1] == '.') {
            ipTermType = 'prefix';
        }

        // Alert histogram.
        this.api.reportHistogram({
            timeRange: range,
            interval: this.reportsService.histogramTimeInterval(range),
            addressFilter: this.ip,
            queryString: this.queryString,
            eventType: 'alert',
            sensorFilter: this.sensorFilter,
        }).then((response: any) => {
            this.alertsOverTime = response.data.map((x: any) => {
                return {
                    date: moment(x.key).toDate(),
                    value: x.count,
                };
            });
        });

        let query = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: 'event_type'}},
                    ],
                    should: [
                        termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                        termQuery(ipTermType, this.asKeyword('dest_ip'), this.ip),
                    ],
                    'minimum_should_match': 1
                }
            },
            size: 0,
            sort: [
                {'@timestamp': {order: 'desc'}}
            ],
            aggs: {

                sensors: {
                    terms: {
                        field: this.asKeyword('host'),
                        size: 1000,
                    },
                },

                alerts: {
                    filter: {
                        term: {event_type: 'alert'}
                    },
                    aggs: {
                        signatures: {
                            terms: {
                                field: this.asKeyword('alert.signature'),
                                size: 10,
                            }
                        }
                    }
                },

                // Top DNS requests made by this IP.
                dnsRequests: {
                    filter: {
                        bool: {
                            filter: [
                                {term: {'event_type': 'dns'}},
                                {term: {'dns.type': 'query'}},
                                termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                            ]
                        },
                    },
                    aggs: {
                        rrnames: {
                            terms: {
                                field: this.asKeyword('dns.rrname'),
                                size: 10,
                            }
                        }
                    }
                },

                // HTTP user agents.
                httpRequests: {
                    filter: {
                        bool: {
                            filter: [
                                {term: {'event_type': 'http'}},
                                termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                            ]
                        }
                    },
                    aggs: {
                        userAgents: {
                            terms: {
                                field: this.asKeyword('http.http_user_agent'),
                                size: 10,
                            }
                        },
                        hostnames: {
                            terms: {
                                field: this.asKeyword('http.hostname'),
                                size: 10,
                            }
                        }
                    }
                },

                http: {
                    filter: {
                        term: {event_type: 'http'},
                    },
                    aggs: {
                        dest: {
                            filter: termQuery(ipTermType, this.asKeyword('dest_ip'), this.ip),
                            aggs: {
                                hostnames: {
                                    terms: {
                                        field: this.asKeyword('http.hostname'),
                                        size: 10,
                                    },
                                }
                            }
                        },
                    }
                },

                // TLS SNI...
                tlsSni: {
                    filter: {
                        bool: {
                            filter: [
                                {term: {'event_type': 'tls'}},
                                termQuery(ipTermType, this.asKeyword('dest_ip'), this.ip),
                            ]
                        }
                    },
                    aggs: {
                        sni: {
                            terms: {
                                field: this.asKeyword('tls.sni'),
                                size: 100,
                            }
                        }
                    }
                },

                // TLS (Versions)...
                tls: {
                    filter: {
                        term: {event_type: 'tls'}
                    },
                    aggs: {
                        asSource: {
                            filter: termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                            aggs: {
                                versions: {
                                    terms: {
                                        field: this.asKeyword('tls.version'),
                                        size: 10,
                                    }
                                },
                                sni: {
                                    terms: {
                                        field: this.asKeyword('tls.sni'),
                                        size: 10,
                                    }
                                },
                                subjects: {
                                    terms: {
                                        field: this.asKeyword('tls.subject'),
                                        size: 10,
                                    }
                                }
                            }
                        },
                        asDest: {
                            filter: termQuery(ipTermType, this.asKeyword('dest_ip'), this.ip),
                            aggs: {
                                versions: {
                                    terms: {
                                        field: this.asKeyword('tls.version'),
                                        size: 10,
                                    }
                                }
                            }
                        }
                    }
                },

                ssh: {
                    filter: {
                        term: {event_type: 'ssh'},
                    },
                    aggs: {
                        // SSH connections as client.
                        sources: {
                            filter: termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                            aggs: {
                                outboundClientProtoVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.client.proto_version'),
                                        size: 10,
                                    }
                                },
                                outboundServerProtoVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.server.proto_version'),
                                        size: 10,
                                    }
                                },
                                // Outbound server versions - that is, the server
                                // versions connected to by this host.
                                outboundServerVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.server.software_version'),
                                        size: 10,
                                    }
                                },
                                // Outbound client versions.
                                outboundClientVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.client.software_version'),
                                        size: 10,
                                    }
                                }
                            }
                        },
                        // SSH connections as server.
                        dests: {
                            filter: termQuery(ipTermType, this.asKeyword('dest_ip'), this.ip),
                            aggs: {
                                inboundClientProtoVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.client.proto_version'),
                                        size: 10,
                                    }
                                },
                                inboundServerProtoVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.server.proto_version'),
                                        size: 10,
                                    }
                                },
                                // Inbound client versions.
                                inboundClientVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.client.software_version'),
                                        size: 10,
                                    }
                                },
                                // Inbound server versions.
                                inboundServerVersions: {
                                    terms: {
                                        field: this.asKeyword('ssh.server.software_version'),
                                        size: 10,
                                    }
                                }
                            }
                        }
                    }
                },

                // Number of flows as client.
                sourceFlows: {
                    filter: {
                        bool: {
                            filter: [
                                {term: {'event_type': 'flow'}},
                                termQuery(ipTermType, this.asKeyword('src_ip'), this.ip),
                            ]
                        }
                    },
                    aggs: {
                        bytesToClient: {
                            sum: {
                                field: 'flow.bytes_toclient',
                            }
                        },
                        bytesToServer: {
                            sum: {
                                field: 'flow.bytes_toserver',
                            }
                        },
                        packetsToClient: {
                            sum: {
                                field: 'flow.pkts_toclient',
                            }
                        },
                        packetsToServer: {
                            sum: {
                                field: 'flow.pkts_toserver',
                            }
                        },
                    }
                },

                // Number of flows as server.
                destFlows: {
                    filter: {
                        bool: {
                            filter: [
                                {term: {'event_type': 'flow'}},
                                termQuery(ipTermType, 'dest_ip', this.ip),
                            ]
                        }
                    },
                    aggs: {
                        bytesToClient: {
                            sum: {
                                field: 'flow.bytes_toclient',
                            }
                        },
                        bytesToServer: {
                            sum: {
                                field: 'flow.bytes_toserver',
                            }
                        },
                        packetsToClient: {
                            sum: {
                                field: 'flow.pkts_toclient',
                            }
                        },
                        packetsToServer: {
                            sum: {
                                field: 'flow.pkts_toserver',
                            }
                        }
                    }
                },

            }
        };

        if (this.sensorFilter != '') {
            this.elasticsearch.addSensorNameFilter(query, this.sensorFilter);
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response) => {

            this.flow.bytesFromIp = response.aggregations.destFlows.bytesToClient.value +
                response.aggregations.sourceFlows.bytesToServer.value;
            this.flow.bytesFromIp = humanizeFileSize(this.flow.bytesFromIp);
            this.flow.bytesToIp = response.aggregations.destFlows.bytesToServer.value +
                response.aggregations.sourceFlows.bytesToClient.value;
            this.flow.bytesToIp = humanizeFileSize(this.flow.bytesToIp);
            this.flow.packetsFromIp = response.aggregations.destFlows.packetsToClient.value +
                response.aggregations.sourceFlows.packetsToServer.value;
            this.flow.packetsToIp = response.aggregations.destFlows.packetsToServer.value +
                response.aggregations.sourceFlows.packetsToClient.value;
            this.flow.sourceFlowCount = response.aggregations.sourceFlows.doc_count;
            this.flow.destFlowCount = response.aggregations.destFlows.doc_count;
            this.flow.ready = true;

            this.userAgents = this.mapTerms(response.aggregations.httpRequests.userAgents.buckets);

            this.topHttpHostnames = this.mapTerms(response.aggregations.httpRequests.hostnames.buckets);

            this.tlsSni = this.mapTerms(response.aggregations.tlsSni.sni.buckets);

            this.tlsClientVersions = this.mapTerms(response.aggregations.tls.asSource.versions.buckets);

            this.tlsServerVersions = this.mapTerms(response.aggregations.tls.asDest.versions.buckets);

            this.topTlsSniRequests = this.mapTerms(response.aggregations.tls.asSource.sni.buckets);

            this.topTlsSubjectRequests = this.mapTerms(response.aggregations.tls.asSource.subjects.buckets);

            this.topDestinationHttpHostnames = this.mapTerms(response.aggregations.http.dest.hostnames.buckets);

            this.topSignatures = this.mapTerms(response.aggregations.alerts.signatures.buckets);

            this.ssh.sshInboundClientVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundClientVersions.buckets);
            this.ssh.sshOutboundClientVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundClientVersions.buckets);
            this.ssh.sshOutboundServerVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundServerVersions.buckets);
            this.ssh.sshInboundServerVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundServerVersions.buckets);
            this.ssh.sshInboundClientProtoVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundClientProtoVersions.buckets);
            this.ssh.sshInboundServerProtoVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundServerProtoVersions.buckets);
            this.ssh.sshOutboundClientProtoVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundClientProtoVersions.buckets);
            this.ssh.sshOutboundServerProtoVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundServerProtoVersions.buckets);

            response.aggregations.sensors.buckets.forEach((bucket: any) => {
                this.sensors.add(bucket.key);
            });

            this.dnsRequestedHostnames = this.mapTerms(
                response.aggregations.dnsRequests.rrnames.buckets);

            this.loading--;
        });

    }

    /**
     * Helper function to map terms aggregations into the common format used
     * by Evebox.
     */
    mapTerms(buckets: any): any[] {
        return buckets.map((bucket: any) => {
            return {
                key: bucket.key,
                count: bucket.doc_count,
            };
        });
    }
}