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

import { Component, Input, OnChanges, OnDestroy, OnInit } from "@angular/core";
import { ReportsService } from "../reports.service";
import { AppEventCode, AppService } from "../../app.service";
import { ActivatedRoute, Params } from "@angular/router";
import { EveboxSubscriptionService } from "../../subscription.service";
import { loadingAnimation } from "../../animations";
import { EveboxSubscriptionTracker } from "../../subscription-tracker";
import { ApiService, ReportAggOptions } from "../../api.service";
import { TopNavService } from "../../topnav.service";
import * as moment from "moment";
import { ElasticSearchService } from "../../elasticsearch.service";
import * as palette from "google-palette";

import { Chart } from 'chart.js';
import { getCanvasElementById } from "../../shared/chartjs";

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
    templateUrl: "ssh-report.component.html",
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

    charts: { [index: string]: any } = {};

    topDestinationAddresses: any[] = [];
    topSourceAddresses: any[] = [];

    constructor(private appService: AppService,
                private ss: EveboxSubscriptionService,
                private route: ActivatedRoute,
                private reports: ReportsService,
                private api: ApiService,
                private topNavService: TopNavService) {
    }

    ngOnInit(): void {

        if (this.route.snapshot.queryParams.q) {
            this.queryString = this.route.snapshot.queryParams.q;
        }

        this.subTracker.subscribe(this.route.queryParams, (params: Params) => {
            this.queryString = params.q || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event: any) => {
            if (event.event === AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy(): void {
        this.subTracker.unsubscribe();
    }

    load(fn: any): void {
        this.loading++;
        fn().then(() => {
        }).catch((err) => {
            console.log("Caught error loading resource:");
            console.log(err);
        }).then(() => {
            this.loading--;
        });
    }


    refresh(): void {

        const size = 100;

        const timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        const aggOptions: ReportAggOptions = {
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
                for (const item of response.data) {
                    const count = item.count;
                    this.eventsOverTime.labels.push(moment(item.key).toDate());
                    this.eventsOverTime.values.push(count);
                    if (count > 0) {
                        nonZeroCount += 1;
                    }
                }

                if (nonZeroCount === 0) {
                    this.eventsOverTime = {
                        labels: [],
                        values: [],
                    };
                }

                setTimeout(() => {
                    const ctx = getCanvasElementById("eventsOverTimeChart");

                    if (this.charts.eventsOverTimeChart) {
                        this.charts.eventsOverTimeChart.destroy();
                    }

                    this.charts.eventsOverTimeChart = new Chart(ctx, {
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
                            plugins: {
                                legend: {
                                    display: false,
                                },
                                title: {
                                    display: true,
                                    text: "SSH Connections Over Time",
                                    padding: 0,
                                }
                            },
                            scales: {
                                x: {
                                    type: "time",
                                },
                            },
                            layout: {},
                            maintainAspectRatio: false,
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

                    this.renderPieChart("serverVersionsPie", versions);
                });
        });

    }

    renderPieChart(canvasId: string, data: any[]): void {
        const labels: string[] = [];
        const values: number[] = [];

        data.forEach((version: any) => {
            labels.push(version.key);
            values.push(version.count);
        });

        const ctx = getCanvasElementById(canvasId);

        if (this.charts[canvasId]) {
            this.charts[canvasId].destroy();
        }

        const colours = this.getColours(data.length);

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
                // legend: {
                //     display: true,
                //     position: "right",
                // },
            }
        });

    }

    private getColours(count: number): string[] {
        const colours = palette("qualitative", count);
        return colours.map(colour => {
            return "#" + colour;
        });
    }

}
