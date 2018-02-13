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

import {Component, OnDestroy, OnInit} from "@angular/core";
import {ActivatedRoute, Params} from "@angular/router";
import {ReportsService} from "./reports.service";
import {AppEvent, AppEventCode, AppService} from "../app.service";
import {TopNavService} from "../topnav.service";
import {ElasticSearchService} from "../elasticsearch.service";
import {loadingAnimation} from "../animations";
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ApiService, ReportAggOptions} from "../api.service";

import * as chartjs from "../shared/chartjs";
import * as moment from "moment";
import {EveBoxProtoPrettyPrinter} from "../pipes/proto-pretty-printer.pipe";

declare var Chart: any;

@Component({
    template: `
      <div class="content" [@loadingState]="(loading > 0) ? 'true' : 'false'">

        <br/>

        <loading-spinner [loading]="loading > 0"></loading-spinner>

        <div class="row">
          <div class="col-md-6 col-sm-6">
            <button type="button" class="btn btn-secondary" (click)="refresh()">
              Refresh
            </button>
          </div>
          <div class="col-md-6 col-sm-6">
            <evebox-filter-input
                [queryString]="queryString"></evebox-filter-input>
          </div>
        </div>

        <br/>

        <div class="row">
          <div class="col">
            <div style="height: 250px;">
              <canvas id="eventsOverTimeChart"
                      style="padding-top: 0px;"></canvas>
            </div>
            <div *ngIf="interval != ''" class="dropdown"
                 style="text-align:center;">
              <span class="mx-auto" data-toggle="dropdown">
                <small><a
                    href="javascript:void(0)">{{interval}} intervals</a></small>
              </span>
              <div class="dropdown-menu">
                <a class="dropdown-item" href="javascript:void(0);"
                   (click)="changeHistogramInterval(item.value)"
                   *ngFor="let item of histogramIntervals">{{item.msg}}</a>
              </div>
            </div>
          </div>
        </div>

        <br/>

        <div class="row">

          <div class="col-md-6">
            <report-data-table *ngIf="topClientsByFlows"
                               title="Top Clients By Flow Count"
                               [rows]="topClientsByFlows"
                               [headers]="['Flows', 'Client IP']"></report-data-table>
          </div>

          <div class="col-md-6">
            <report-data-table *ngIf="topServersByFlows"
                               title="Top Servers By Flow Count"
                               [rows]="topServersByFlows"
                               [headers]="['Flows', 'Server IP']"></report-data-table>
          </div>

        </div>

        <br/>

        <div *ngIf="topFlowsByAge" class="card">
          <div class="card-header">
            <b>Top Flows by Age</b>
          </div>
          <eveboxEventTable2 [rows]="topFlowsByAge"
                             [showEventType]="false"
                             [showActiveEvent]="false"></eveboxEventTable2>
        </div>

        <br/>

      </div>`,
    animations: [
        loadingAnimation,
    ]
})
export class FlowReportComponent implements OnInit, OnDestroy {

    topClientsByFlows: any[];
    topServersByFlows: any[];

    topFlowsByAge: any[];

    loading = 0;

    queryString = "";

    subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    private charts: any = {};

    histogramIntervals: any[] = [
        {value: "1s", msg: "1 second"},
        {value: "1m", msg: "1 minute"},
        {value: "5m", msg: "5 minute"},
        {value: "15m", msg: "15 minutes"},
        {value: "1h", msg: "1 hour"},
        {value: "6h", msg: "6 hours"},
        {value: "12h", msg: "12 hours"},
        {value: "1d", msg: "1 day (24 hours)"},
    ];

    public interval: string = null;

    private range: number = null;

    constructor(private appService: AppService,
                private route: ActivatedRoute,
                private reportsService: ReportsService,
                private topNavService: TopNavService,
                private api: ApiService,
                private elasticsearch: ElasticSearchService,
                private protoPrettyPrinter: EveBoxProtoPrettyPrinter) {
    }

    ngOnInit() {

        this.range = this.topNavService.getTimeRangeAsSeconds();

        this.subTracker.subscribe(this.route.params, (params: Params) => {
            this.queryString = params["q"] || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event: AppEvent) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.range = this.topNavService.getTimeRangeAsSeconds();
                this.interval = null;
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.subTracker.unsubscribe();
    }

    load(fn: any) {
        this.loading++;
        fn().then(() => {
        }).catch((err) => {
        }).then(() => {
            this.loading--;
        });
    }

    private renderChart(id: string, options: any) {
        let element = document.getElementById(id);
        let ctx = (<HTMLCanvasElement>element).getContext("2d");

        if (this.charts[id]) {
            this.charts[id].destroy();
        }

        this.charts[id] = new Chart(ctx, options);
    }

    private refreshEventsOverTime() {
        if (!this.interval) {
            this.interval = "1d";

            if (this.range <= 60) {
                this.interval = "1s";
            } else if (this.range <= 3600) {
                this.interval = "1m";
            } else if (this.range <= (3600 * 3)) {
                this.interval = "5m";
            } else if (this.range <= 3600 * 24) {
                this.interval = "15m";
            } else if (this.range <= 3600 * 24 * 3) {
                this.interval = "1h";
            }
        }

        let histogramOptions: any = {
            appProto: true,
            queryString: this.queryString,
            interval: this.interval,
        };

        if (this.range > 0) {
            histogramOptions.timeRange = `${this.range}s`;
        }

        this.api.flowHistogram(histogramOptions).subscribe((response) => {
            console.log(response);

            let labels = [];
            let eventCounts = [];
            let protos = [];

            response.data.forEach((elem) => {
                for (let proto in elem.app_proto) {
                    if (protos.indexOf(proto) < 0) {
                        protos.push(proto);
                    }
                }
            });

            let data = {};

            let colours = chartjs.getColourPalette(protos.length + 1);

            let totals = [];

            response.data.forEach((elem) => {
                let proto_sum = 0;
                for (let proto of protos) {
                    if (!data[proto]) {
                        data[proto] = [];
                    }
                    if (proto in elem.app_proto) {
                        let val = elem.app_proto[proto];
                        data[proto].push(val);
                        proto_sum += val;
                    } else {
                        data[proto].push(0);
                    }
                }
                labels.push(moment(elem.key).toDate());

                totals.push(elem.events);
                eventCounts.push(elem.events - proto_sum);
            });

            let datasets: any[] = [{
                label: "Other",
                backgroundColor: colours[0],
                borderColor: colours[0],
                data: eventCounts,
                fill: false,
            }];

            let i = 1;

            for (let proto of protos) {
                let label = proto;
                if (proto === "failed") {
                    label = "Unknown";
                } else {
                    label = this.protoPrettyPrinter.transform(proto, null);
                }
                datasets.push({
                    label: label,
                    backgroundColor: colours[i],
                    borderColor: colours[i],
                    fill: false,
                    data: data[proto],
                });
                i += 1;
            }

            this.renderChart("eventsOverTimeChart", {
                type: "bar",
                data: {
                    labels: labels,
                    datasets: datasets,
                },
                options: {
                    title: {
                        display: true,
                        text: "Flow Events Over Time",
                        padding: 0,
                    },
                    legend: {
                        position: "right",
                    },
                    scales: {
                        xAxes: [
                            {
                                display: true,
                                type: "time",
                                stacked: true,
                            }
                        ],
                        yAxes: [
                            {
                                gridLines: false,
                                stacked: true,
                                ticks: {
                                    padding: 5,
                                }
                            }
                        ]
                    },
                    maintainAspectRatio: false,
                    responsive: true,
                    tooltips: {
                        callbacks: {
                            footer: function (a, b) {
                                let index = a[0].index;
                                return `Total: ${totals[index]}`;
                            }
                        }
                    }
                }
            });
        });
    }

    changeHistogramInterval(interval) {
        this.interval = interval;
        this.refreshEventsOverTime();
    }

    refresh() {
        let now = moment();

        this.refreshEventsOverTime();

        let aggOptions: ReportAggOptions = {
            timeRange: this.range,
            eventType: "flow",
            size: 10,
            queryString: this.queryString,
        };

        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response: any) => {
                    this.topClientsByFlows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response: any) => {
                    this.topServersByFlows = response.data;
                });
        });

        let query: any = {
            query: {
                bool: {
                    filter: [
                        // Somewhat limit to eve events only.
                        {exists: {field: "event_type"}},
                        {term: {event_type: "flow"}}
                    ]
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                topFlowsByAge: {
                    top_hits: {
                        sort: [
                            {
                                "flow.age": {
                                    order: "desc",
                                    unmapped_type: "long"
                                }
                            }
                        ],
                        size: 10,
                    }
                }
            }
        };

        // if (this.queryString && this.queryString != "") {
        //     query.query.filtered.query = {
        //         query_string: {
        //             query: this.queryString
        //         }
        //     };
        // }

        this.elasticsearch.addTimeRangeFilter(query, now, this.range);

        this.load(() => {
            return this.elasticsearch.search(query).then((response: any) => {
                this.topFlowsByAge = response.aggregations.topFlowsByAge.hits.hits;
                this.loading--;
            });
        });

    }
}