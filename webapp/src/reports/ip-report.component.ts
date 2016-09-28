import {Component, OnInit, OnDestroy} from "@angular/core";
import {ActivatedRoute} from "@angular/router";
import {EveboxSubscriptionService} from "../subscription.service";
import {ElasticSearchService} from "../elasticsearch.service";
import {TopNavService} from "../topnav.service";

import moment = require("moment");
import {ReportsService} from "./reports.service";
import {AppService, AppEvent, AppEventCode} from "../app.service";

@Component({
    template: `<div *ngIf="ip">

  <h2>Report for IP {{ip}}</h2>

  <metrics-graphic *ngIf="eventsOverTime"
                   graphId="eventsOverTime"
                   title="Activity Over Time"
                   [data]="eventsOverTime">
  </metrics-graphic>

  <div class="row">

    <!-- First Column -->
    <div class="col-md-6">

      <report-data-table *ngIf="dnsRequestsByHostname"
                         title="DNS Hostnames Returning {{ip}} (Total: {{dnsRequestCount}})"
                         [rows]="dnsRequestsByHostname"
                         [headers]="['#', 'Hostname']"></report-data-table>

      <report-data-table *ngIf="userAgents"
                         title="Outgoing HTTP User Agents"
                         [rows]="userAgents"
                         [headers]="['#', 'User Agent']"></report-data-table>

    </div>

    <!-- Second Column -->
    <div class="col-md-6">

      <div class="panel panel-default">
        <div class="panel-heading">
          <b>Flow</b>
        </div>
        <table class="table">
          <tbody>
          <tr>
            <td>Flows As Client</td>
            <td>{{sourceFlowCount}}</td>
          </tr>
          <tr>
            <td>Flows As Server</td>
            <td>{{destFlowCount}}</td>
          </tr>
          <tr>
            <td>Bytes To...</td>
            <td>{{bytesToIp}}</td>
          </tr>
          <tr>
            <td>Bytes From...</td>
            <td>{{bytesFromIp}}</td>
          </tr>
          </tbody>
        </table>
      </div> <!-- end panel -->

      <report-data-table *ngIf="tlsSni"
                         title="Incoming TLS Server Names (SNI)"
                         [rows]="tlsSni"
                         [headers]="['#', 'Name']"></report-data-table>

      <div class="row">
        <div class="col-md-6">
          <report-data-table *ngIf="tlsClientVersions"
                             title="TLS Versions as Client"
                             [rows]="tlsClientVersions"
                             [headers]="['#', 'Version']"></report-data-table>
        </div>
        <div class="col-md-6">
          <report-data-table *ngIf="tlsServerVersions"
                             title="TLS Versions as Server"
                             [rows]="tlsServerVersions"
                             [headers]="['#', 'Version']"></report-data-table>
        </div>
      </div>

    </div>

  </div>

</div>`
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

    private tlsSni:any[];

    private tlsClientVersions:any[];
    private tlsServerVersions:any[];

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
                        }
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
                                        size: 100,
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
                                        size: 100,
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

            let userAgents:any = response.aggregations.httpRequests.userAgents.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });
            this.userAgents = userAgents;

            let tlsSni:any = response.aggregations.tlsSni.sni.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });
            this.tlsSni = tlsSni;

            let tlsClientVersions = response.aggregations.tls.asSource.versions.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });
            this.tlsClientVersions = tlsClientVersions;

            let tlsServerVersions = response.aggregations.tls.asDest.versions.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                }
            });
            this.tlsServerVersions = tlsServerVersions;

        });

    }
}