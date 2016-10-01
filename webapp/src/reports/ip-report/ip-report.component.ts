import {Component, OnInit, OnDestroy} from "@angular/core";
import {ActivatedRoute} from "@angular/router";
import {EveboxSubscriptionService} from "../../subscription.service";
import {ElasticSearchService} from "../../elasticsearch.service";
import {TopNavService} from "../../topnav.service";

import moment = require("moment");
import {ReportsService} from "../reports.service";
import {AppService, AppEvent, AppEventCode} from "../../app.service";
import {Input} from "@angular/core";

@Component({
    selector: "requestedHostnamesForIp",
    template: `
      <report-data-table *ngIf="topRrnames"
                         title="DNS: Top Requested Hostnames By {{address}}"
                         [rows]="topRrnames"
                         [headers]="['#', 'Hostname']"></report-data-table>
`
})
export class RequestedHostnamesForIpComponent implements OnInit, OnDestroy {

    @Input() private address:string;
    @Input() private count:number = 10;

    private topRrnames:any[];

    constructor(private elasticsearch:ElasticSearchService,
                private topNavService:TopNavService) {
    }

    ngOnInit() {
        this.refresh();
    }

    ngOnDestroy() {
    }

    refresh() {

        console.log("Loading top DNS requests for " + this.address);

        let now = moment();
        let range = this.topNavService.getTimeRangeAsSeconds();

        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {term: {"event_type": "dns"}},
                            {term: {"dns.type": "query"}},
                            {term: {"src_ip.raw": this.address}},
                        ]
                    }
                }
            },
            aggs: {
                rrnames: {
                    terms: {
                        field: "dns.rrname.raw",
                        size: this.count,
                    }
                }
            }
        };

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response:any) => {
            console.log("DNS requests:");
            console.log(response);

            let returnCount = response.aggregations.rrnames.buckets.length;
            let total = response.aggregations.rrnames.sum_other_doc_count;

            console.log(`Total: ${total}; Returned: ${returnCount}`);

            let topRrnames:any[] = response.aggregations.rrnames.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });
            this.topRrnames = topRrnames;
        })
    }
}

@Component({
    templateUrl: "./ip-report.component.html",
})
export class IpReportComponent implements OnInit, OnDestroy {

    private ip:string;

    private eventsOverTime:any[];

    // Number of flows as client.
    private sourceFlowCount:number;

    // Number of flows as server.
    private destFlowCount:number;

    // Number of DNS requests returning this IP.
    private dnsRequestCount:number;

    private dnsRequestsByHostname:any[];

    private bytesToIp:number;

    private bytesFromIp:number;

    private userAgents:any[];

    private topHttpHostnames:any[];

    private tlsSni:any[];

    private topTlsSniRequests:any[];

    private tlsClientVersions:any[];

    private tlsServerVersions:any[];

    private topTlsSubjectRequests:any[];

    private topDestinationHttpHostnames:any[];

    private topSignatures:any[];

    private sshInboundClientVersions:any[];

    private sshOutboundClientVersions:any[];

    private sshOutboundServerVersions:any[];

    private sshInboundServerVersions:any[];

    private sshOutboundClientProtoVersions:any[];

    private sshOutboundServerProtoVersions:any[]

    private sshInboundClientProtoVersions:any[];

    private sshInboundServerProtoVersions:any[]

    constructor(private route:ActivatedRoute,
                private elasticsearch:ElasticSearchService,
                private appService:AppService,
                private topNavService:TopNavService,
                private reportsService:ReportsService,
                private ss:EveboxSubscriptionService) {
    }

    ngOnInit() {
        this.ss.subscribe(this, this.route.params, (params:any) => {
            this.ip = params.ip;
            this.refresh();
        });

        this.ss.subscribe(this, this.appService, (event:AppEvent) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });
    }

    ngOnDestroy() {
        this.ss.unsubscribe(this);
    }

    loadDnsInfo(range:any, now:any) {
        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {"event_type": "dns"}},
                            {term: {"dns.type.raw": "answer"}},
                            {term: {"dns.rdata.raw": this.ip}},
                        ]
                    }
                }
            },
            size: 0,
            aggs: {

                uniqueHostnames: {
                    terms: {
                        field: "dns.rrname.raw",
                        size: 100,
                    }
                }

            }
        };

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response:any) => {

            this.dnsRequestCount = response.hits.total;

            this.dnsRequestsByHostname = response.aggregations.uniqueHostnames.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });

        });
    }

    refresh() {

        let range = this.topNavService.getTimeRangeAsSeconds();
        let now = moment();

        this.loadDnsInfo(range, now);

        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {
                                or: [
                                    {term: {"src_ip.raw": this.ip}},
                                    {term: {"dest_ip.raw": this.ip}}
                                ]
                            }
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {

                alerts: {
                    filter: {
                        term: {event_type: "alert"}
                    },
                    aggs: {
                        signatures: {
                            terms: {
                                field: "alert.signature.raw",
                                size: 10,
                            }
                        }
                    }
                },

                // HTTP user agents.
                httpRequests: {
                    filter: {
                        and: [
                            {term: {"event_type": "http"}},
                            {term: {"src_ip.raw": this.ip}},
                        ]
                    },
                    aggs: {
                        userAgents: {
                            terms: {
                                field: "http.http_user_agent.raw",
                                size: 10,
                            }
                        },
                        hostnames: {
                            terms: {
                                field: "http.hostname.raw",
                                size: 10,
                            }
                        }
                    }
                },

                http: {
                    filter: {
                        term: {event_type: "http"},
                    },
                    aggs: {
                        dest: {
                            filter: {
                                term: {"dest_ip.raw": this.ip}
                            },
                            aggs: {
                                hostnames: {
                                    terms: {
                                        field: "http.hostname.raw",
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
                        and: [
                            {term: {"event_type": "tls"}},
                            {term: {"dest_ip.raw": this.ip}},
                        ]
                    },
                    aggs: {
                        sni: {
                            terms: {
                                field: "tls.sni.raw",
                                size: 100,
                            }
                        }
                    }
                },

                // TLS (Versions)...
                tls: {
                    filter: {
                        term: {event_type: "tls"}
                    },
                    aggs: {
                        asSource: {
                            filter: {
                                term: {"src_ip.raw": this.ip}
                            },
                            aggs: {
                                versions: {
                                    terms: {
                                        field: "tls.version.raw",
                                        size: 10,
                                    }
                                },
                                sni: {
                                    terms: {
                                        field: "tls.sni.raw",
                                        size: 10,
                                    }
                                },
                                subjects: {
                                    terms: {
                                        field: "tls.subject.raw",
                                        size: 10,
                                    }
                                }
                            }
                        },
                        asDest: {
                            filter: {
                                term: {"dest_ip.raw": this.ip}
                            },
                            aggs: {
                                versions: {
                                    terms: {
                                        field: "tls.version.raw",
                                        size: 10,
                                    }
                                }
                            }
                        }
                    }
                },

                ssh: {
                    filter: {
                        term: {event_type: "ssh"},
                    },
                    aggs: {
                        // SSH connections as client.
                        sources: {
                            filter: {
                                term: {"src_ip.raw": this.ip}
                            },
                            aggs: {
                                outboundClientProtoVersions: {
                                  terms: {
                                      field: "ssh.client.proto_version.raw",
                                      size:10,
                                  }
                                },
                                outboundServerProtoVersions: {
                                    terms: {
                                        field: "ssh.server.proto_version.raw",
                                        size:10,
                                    }
                                },
                                // Outbound server versions - that is, the server
                                // versions connected to by this host.
                                outboundServerVersions: {
                                    terms: {
                                        field: "ssh.server.software_version.raw",
                                        size: 10,
                                    }
                                },
                                // Outbound client versions.
                                outboundClientVersions: {
                                    terms: {
                                        field: "ssh.client.software_version.raw",
                                        size: 10,
                                    }
                                }
                            }
                        },
                        // SSH connections as server.
                        dests: {
                            filter: {
                                term: {"dest_ip.raw": this.ip}
                            },
                            aggs: {
                                inboundClientProtoVersions: {
                                    terms: {
                                        field: "ssh.client.proto_version.raw",
                                        size:10,
                                    }
                                },
                                inboundServerProtoVersions: {
                                    terms: {
                                        field: "ssh.server.proto_version.raw",
                                        size:10,
                                    }
                                },
                                // Inbound client versions.
                                inboundClientVersions: {
                                    terms: {
                                        field: "ssh.client.software_version.raw",
                                        size: 10,
                                    }
                                },
                                // Inbound server versions.
                                inboundServerVersions: {
                                    terms: {
                                        field: "ssh.server.software_version.raw",
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
                        and: [
                            {term: {"event_type": "flow"}},
                            {term: {"src_ip.raw": this.ip}},
                        ]
                    },
                    aggs: {
                        bytesToClient: {
                            sum: {
                                field: "flow.bytes_toclient",
                            }
                        },
                        bytesToServer: {
                            sum: {
                                field: "flow.bytes_toserver",
                            }
                        }
                    }
                },

                // Number of flows as server.
                destFlows: {
                    filter: {
                        and: [
                            {term: {"event_type": "flow"}},
                            {term: {"dest_ip.raw": this.ip}},
                        ]
                    },
                    aggs: {
                        bytesToClient: {
                            sum: {
                                field: "flow.bytes_toclient",
                            }
                        },
                        bytesToServer: {
                            sum: {
                                field: "flow.bytes_toserver",
                            }
                        }
                    }
                },

            }
        };

        this.elasticsearch.addTimeRangeFilter(query, now, range);
        this.reportsService.addEventsOverTimeAggregation(query, now, range);

        this.elasticsearch.search(query).then((response) => {
            console.log(response);

            this.eventsOverTime = response.aggregations.events_over_time.buckets.map((bucket:any) => {
                return {
                    date: moment(bucket.key).toDate(),
                    value: bucket.doc_count,
                }
            });

            this.bytesFromIp = response.aggregations.destFlows.bytesToClient.value +
                response.aggregations.sourceFlows.bytesToServer.value;
            this.bytesToIp = response.aggregations.destFlows.bytesToServer.value +
                response.aggregations.sourceFlows.bytesToClient.value;

            this.sourceFlowCount = response.aggregations.sourceFlows.doc_count;
            this.destFlowCount = response.aggregations.destFlows.doc_count;

            this.userAgents = this.mapTerms(response.aggregations.httpRequests.userAgents.buckets);

            this.topHttpHostnames = this.mapTerms(response.aggregations.httpRequests.hostnames.buckets);

            this.tlsSni = this.mapTerms(response.aggregations.tlsSni.sni.buckets);

            this.tlsClientVersions = this.mapTerms(response.aggregations.tls.asSource.versions.buckets);

            this.tlsServerVersions = this.mapTerms(response.aggregations.tls.asDest.versions.buckets);

            this.topTlsSniRequests = this.mapTerms(response.aggregations.tls.asSource.sni.buckets);

            this.topTlsSubjectRequests = this.mapTerms(response.aggregations.tls.asSource.subjects.buckets);

            this.topDestinationHttpHostnames = this.mapTerms(response.aggregations.http.dest.hostnames.buckets);

            this.topSignatures = this.mapTerms(response.aggregations.alerts.signatures.buckets);

            this.sshInboundClientVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundClientVersions.buckets);

            this.sshOutboundClientVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundClientVersions.buckets);

            this.sshOutboundServerVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundServerVersions.buckets);

            this.sshInboundServerVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundServerVersions.buckets);

            this.sshInboundClientProtoVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundClientProtoVersions.buckets);

            this.sshInboundServerProtoVersions = this.mapTerms(
                response.aggregations.ssh.dests.inboundServerProtoVersions.buckets);

            this.sshOutboundClientProtoVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundClientProtoVersions.buckets);

            this.sshOutboundServerProtoVersions = this.mapTerms(
                response.aggregations.ssh.sources.outboundServerProtoVersions.buckets);
        });

    }

    /**
     * Helper function to map terms aggregations into the common format used
     * by Evebox.
     */
    private mapTerms(buckets:any):any[] {
        return buckets.map((bucket:any) => {
            return {
                key: bucket.key,
                count: bucket.doc_count,
            }
        });
    }
}