/* Copyright (c) 2017 Jason Ish
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

import {Component, Input, OnChanges, OnDestroy, OnInit} from "@angular/core";
import {ReportsService} from "./reports.service";
import {AppEventCode, AppService} from "../app.service";
import {ActivatedRoute, Params} from "@angular/router";
import {EveboxSubscriptionService} from "../subscription.service";
import {loadingAnimation} from "../animations";
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ApiService, ReportAggOptions} from "../api.service";
import {TopNavService} from "../topnav.service";
import * as moment from "moment";
import {ElasticSearchService} from "../elasticsearch.service";
import * as palette from "google-palette";

declare var Chart: any;

@Component({
    selector: "evebox-ip-addr-data-table",
    template: `
      <report-data-table *ngIf="rows"
                         [title]="title"
                         [rows]="rows"
                         [headers]="headers"></report-data-table>
    `,
})
export class IpAddrDataTableComponent implements OnInit, OnChanges {

    @Input() rows: any[] = [];
    @Input() headers: string[] = [];
    @Input() title: string;

    constructor(private elasticSearch: ElasticSearchService) {
    }

    ngOnInit(): void {
        this.resolveHostnames();
    }

    ngOnChanges(): void {
        this.resolveHostnames();
    }

    resolveHostnames() {
        if (this.rows.length == 0) {
            return;
        }

        console.log(`Resolving hostnames for data table ${this.title}.`);
        this.rows.forEach((result: any) => {
            this.elasticSearch.resolveHostnameForIp(result.key).then((hostname: string) => {
                if (hostname) {
                    result.searchKey = result.key;
                    result.key = `${result.key} (${hostname})`;
                }
            });
        });
    }

}

@Component({
    template: `
      <div class="content" [@loadingState]="(loading > 0) ? 'true' : 'false'">
        <loading-spinner [loading]="loading > 0"></loading-spinner>
        <br/>
        <div class="row">
          <div class="col-sm">
            <button type="button" class="btn btn-secondary"
                    (click)="refresh()"> Refresh
            </button>
          </div>
          <div class="col-sm">
            <evebox-filter-input
                [queryString]="queryString"></evebox-filter-input>
          </div>
        </div>

        <br/>

        <div class="row">
          <div class="col">
            <div style="height: 225px"
                 [hidden]="!eventsOverTime || eventsOverTime.values.length == 0">
              <canvas id="eventsOverTimeChart"></canvas>
            </div>
          </div>
        </div>

        <br/>

        <div class="row">
          <div class="col-md-6">
            <div class="card">
              <div class="card-header"> SSH Client Software</div>
              <div class="card-body">
                <canvas [hidden]="!clientSoftware || clientSoftware.length == 0"
                        id="clientVersionsPie" style="height: 300px;"></canvas>
                <div *ngIf="!clientSoftware || clientSoftware.length == 0">No
                  data.
                </div>
              </div>
            </div>
          </div>
          <div class="col-md-6">
            <div class="card">
              <div class="card-header"> SSH Server Software</div>
              <div class="card-body">
                <canvas [hidden]="!serverSoftware || serverSoftware.length == 0"
                        id="serverVersionsPie" style="height: 300px;"></canvas>
                <div *ngIf="!serverSoftware || serverSoftware.length == 0">No
                  data.
                </div>
              </div>
            </div>
          </div>
        </div>

        <br/>

        <div class="row">
          <div class="col-md-6">
            <report-data-table *ngIf="clientSoftware"
                               title="SSH Client Software"
                               [rows]="clientSoftware"
                               [headers]="['#', 'Software']"></report-data-table>
          </div>
          <div class="col-md-6">
            <report-data-table *ngIf="serverSoftware"
                               title="SSH Server Software"
                               [rows]="serverSoftware"
                               [headers]="['#', 'Software']"></report-data-table>
          </div>
        </div>
        <br/>
        <div class="row">
          <div class="col-sm">
            <evebox-ip-addr-data-table *ngIf="topSourceAddresses"
                                       title="Top SSH Client Hosts"
                                       [rows]="topSourceAddresses"
                                       [headers]="['#', 'Address']"></evebox-ip-addr-data-table>
          </div>
          <div class="col-sm">
            <evebox-ip-addr-data-table *ngIf="topDestinationAddresses"
                                       title="Top SSH Server Hosts"
                                       [rows]="topDestinationAddresses"
                                       [headers]="['#', 'Address']"></evebox-ip-addr-data-table>
          </div>
        </div>
      </div>`,
    animations: [
        loadingAnimation,
    ]
})
export class SshReportComponent implements OnInit, OnDestroy {

    eventsOverTime: any = {
        labels: [],
        values: [],
    };

    serverSoftware: any[] = [];
    clientSoftware: any[] = [];

    loading = 0;

    queryString = "";

    subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    charts: any = {};

    constructor(private appService: AppService,
                private ss: EveboxSubscriptionService,
                private route: ActivatedRoute,
                private reports: ReportsService,
                private api: ApiService,
                private topNavService: TopNavService) {
    }

    ngOnInit() {

        if (this.route.snapshot.queryParams["q"]) {
            this.queryString = this.route.snapshot.queryParams["q"];
        }

        this.subTracker.subscribe(this.route.params, (params: Params) => {
            this.queryString = params["q"] || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event: any) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
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
            console.log("Caught error loading resource:");
            console.log(err);
        }).then(() => {
            this.loading--;
        });
    }

    topDestinationAddresses: any[] = [];
    topSourceAddresses: any[] = [];

    refresh() {

        let size = 100;

        let timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions: ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "ssh",
        };

        // Top source IPs.
        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response: any) => {
                    this.topSourceAddresses = response.data;
                });
        });

        // Top destination IPs.
        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response: any) => {
                    this.topDestinationAddresses = response.data;
                });
        });

        this.load(() => {
            return this.api.reportHistogram({
                timeRange: timeRangeSeconds,
                interval: this.reports.histogramTimeInterval(timeRangeSeconds),
                eventType: "ssh",
                queryString: this.queryString,
            }).then((response: any) => {

                this.eventsOverTime = {
                    labels: [],
                    values: [],
                };

                let nonZeroCount = 0;
                for (let item of response.data) {
                    let count = item.count;
                    this.eventsOverTime.labels.push(moment(item.key).toDate());
                    this.eventsOverTime.values.push(count);
                    if (count > 0) {
                        nonZeroCount += 1;
                    }
                }

                if (nonZeroCount == 0) {
                    this.eventsOverTime = {
                        labels: [],
                        values: [],
                    };
                }

                setTimeout(() => {
                    let ctx = document.getElementById("eventsOverTimeChart");

                    if (this.charts["eventsOverTimeChart"]) {
                        this.charts["eventsOverTimeChart"].destroy();
                    }

                    this.charts["eventsOverTimeChart"] = new Chart(ctx, {
                        type: "bar",
                        data: {
                            labels: this.eventsOverTime.labels,
                            datasets: [
                                {
                                    backgroundColor: this.getColours(1)[0],
                                    data: this.eventsOverTime.values,
                                }
                            ]
                        },
                        options: {
                            response: true,
                            title: {
                                display: true,
                                text: "SSH Connections Over Time",
                            },
                            scales: {
                                xAxes: [
                                    {
                                        type: "time",
                                        distribution: "series",
                                        ticks: {
                                            maxRotation: 0,
                                        },
                                        gridLines: {
                                            tickMarkLength: 10,
                                        }
                                    }
                                ]
                            },
                            legend: {
                                display: false,
                            },
                            layout: {},
                            maintainAspectRatio: false,
                            tooltips: {
                                enabled: true,
                                intersect: false,
                                displayColors: false,
                                callbacks: {
                                    title: function (t) {
                                        return moment(t[0].xLabel).format("YYYY-MM-DD");
                                    }
                                }
                            }
                        },
                    });


                }, 0);

            });
        });

        this.load(() => {
            return this.api.reportAgg("ssh.client.software_version", aggOptions)
                .then((response: any) => {

                    this.clientSoftware = response.data;

                    // Only graph the top 10 then sum up the rest under "Other".
                    const versions: any = [];

                    for (let i = 0; i < response.data.length; i++) {
                        if (i < 10) {
                            versions.push(response.data[i]);
                        }
                        if (i === 10) {
                            versions.push({key: "Other", count: 0});
                        }
                        if (i >= 10) {
                            versions[10].count += response.data[i].count;
                        }
                    }

                    this.renderPieChart("clientVersionsPie", versions);
                });
        });

        this.load(() => {
            return this.api.reportAgg("ssh.server.software_version", aggOptions)
                .then((response: any) => {

                    this.serverSoftware = response.data;

                    // Only graph the top 10 then sum up the rest under "Other".
                    let versions: any = [];

                    for (let i = 0; i < response.data.length; i++) {
                        if (i < 10) {
                            versions.push(response.data[i]);
                        }
                        if (i == 10) {
                            versions.push({key: "Other", count: 0});
                        }
                        if (i >= 10) {
                            versions[10].count += response.data[i].count;
                        }
                    }

                    this.renderPieChart("serverVersionsPie", versions);
                });
        });

    }

    renderPieChart(canvasId: string, data: any[]) {
        let labels: string[] = [];
        let values: number[] = [];

        data.forEach((version: any) => {
            labels.push(version.key);
            values.push(version.count);
        });

        let ctx = document.getElementById(canvasId);

        if (this.charts[canvasId]) {
            this.charts[canvasId].destroy();
        }

        let colours = this.getColours(data.length);

        this.charts[canvasId] = new Chart(ctx, {
            type: "pie",
            data: {
                labels: labels,
                datasets: [
                    {
                        data: values,
                        backgroundColor: colours,
                    }
                ]
            },
            options: {
                legend: {
                    display: true,
                    position: "right",
                },
            }
        });

    }

    private getColours(count: number): string[] {
        let colours = palette("qualitative", count);
        return colours.map(colour => {
            return "#" + colour;
        });
    }

}
