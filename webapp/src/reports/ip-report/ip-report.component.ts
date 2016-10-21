import {Component, OnInit, OnDestroy, Input} from "@angular/core";
import {ActivatedRoute} from "@angular/router";
import {EveboxSubscriptionService} from "../../subscription.service";
import {ElasticSearchService} from "../../elasticsearch.service";
import {TopNavService} from "../../topnav.service";
import {ReportsService} from "../reports.service";
import {AppService, AppEvent, AppEventCode} from "../../app.service";
import {loadingAnimation} from "../../animations";

import moment = require("moment");
import {humanizeFileSize} from "../../humanize.service";

function termQuery(type:string, field:string, value:string) {
    let term = {};
    term[type] = {};
    term[type][field] = value;
    return term;
}

@Component({
    templateUrl: "./ip-report.component.html",
    animations: [
        loadingAnimation,
    ]
})
export class IpReportComponent implements OnInit, OnDestroy {

    private ip:string;

    private loading:number = 0;

    private eventsOverTime:any[];

    // Number of flows as client.
    private sourceFlowCount:number;

    // Number of flows as server.
    private destFlowCount:number;

    // DNS hostname lookups returning this IP.
    private dnsHostnamesForAddress:any[];

    // Top requested hostnames.
    private dnsRequestedHostnames:any[];

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

    private sshInboundServerProtoVersions:any[];

    private sensors:Set<string> = new Set<string>();

    // Empty string defaults to all sensors.
    private sensorFilter:string = "";

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

    queryDnsHostnamesForAddress(range:any, now:any) {

        this.loading++;

        let ipTermType = "term";

        if (this.ip[this.ip.length - 1] == '.') {
            ipTermType = "prefix";
        }

        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {"event_type": "dns"}},
                            {term: {"dns.type.raw": "answer"}},
                            termQuery(ipTermType, "dns.rdata", this.ip),
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

        if (this.sensorFilter != "") {
            this.elasticsearch.addSensorNameFilter(query, this.sensorFilter);
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response:any) => {

            this.dnsHostnamesForAddress = response.aggregations.uniqueHostnames.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });

            this.loading--;

        });
    }

    refresh() {

        let range = this.topNavService.getTimeRangeAsSeconds();
        let now = moment();

        this.queryDnsHostnamesForAddress(range, now);

        this.loading++;

        let ipTermType = "term";

        if (this.ip[this.ip.length - 1] == '.') {
            ipTermType = "prefix";
        }

        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {
                                or: [
                                    termQuery(ipTermType, "src_ip", this.ip),
                                    termQuery(ipTermType, "dest_ip", this.ip),
                                ]
                            },
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {

                sensors: {
                    terms: {
                        field: "host.raw",
                        size: 1000,
                    },
                },

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

                // Top DNS requests made by this IP.
                dnsRequests: {
                    filter: {
                        and: [
                            {term: {"event_type": "dns"}},
                            {term: {"dns.type": "query"}},
                            termQuery(ipTermType, "src_ip", this.ip),
                        ]
                    },
                    aggs: {
                        rrnames: {
                            terms: {
                                field: "dns.rrname.raw",
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
                            termQuery(ipTermType, "src_ip", this.ip),
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
                            filter: termQuery(ipTermType, "dest_ip", this.ip),
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
                            termQuery(ipTermType, "dest_ip", this.ip),
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
                            filter: termQuery(ipTermType, "src_ip", this.ip),
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
                            filter: termQuery(ipTermType, "dest_ip", this.ip),
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
                            filter: termQuery(ipTermType, "src_ip", this.ip),
                            aggs: {
                                outboundClientProtoVersions: {
                                    terms: {
                                        field: "ssh.client.proto_version.raw",
                                        size: 10,
                                    }
                                },
                                outboundServerProtoVersions: {
                                    terms: {
                                        field: "ssh.server.proto_version.raw",
                                        size: 10,
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
                            filter: termQuery(ipTermType, "dest_ip", this.ip),
                            aggs: {
                                inboundClientProtoVersions: {
                                    terms: {
                                        field: "ssh.client.proto_version.raw",
                                        size: 10,
                                    }
                                },
                                inboundServerProtoVersions: {
                                    terms: {
                                        field: "ssh.server.proto_version.raw",
                                        size: 10,
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
                            termQuery(ipTermType, "src_ip", this.ip),
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
                            termQuery(ipTermType, "dest_ip", this.ip),
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

        if (this.sensorFilter != "") {
            this.elasticsearch.addSensorNameFilter(query, this.sensorFilter);
        }

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
            this.bytesFromIp = humanizeFileSize(this.bytesFromIp);

            this.bytesToIp = response.aggregations.destFlows.bytesToServer.value +
                response.aggregations.sourceFlows.bytesToClient.value;
            this.bytesToIp = humanizeFileSize(this.bytesToIp);

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

            response.aggregations.sensors.buckets.forEach((bucket:any) => {
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
    private mapTerms(buckets:any):any[] {
        return buckets.map((bucket:any) => {
            return {
                key: bucket.key,
                count: bucket.doc_count,
            }
        });
    }
}