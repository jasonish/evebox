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

import { Component, OnDestroy, OnInit } from "@angular/core";
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
import { getCanvasElementById, getColourPalette } from "../../shared/chartjs";
import { Chart, ChartConfiguration } from "chart.js";

@Component({
    templateUrl: "alerts-report.component.html",
    animations: [
        loadingAnimation,
    ]
})
export class AlertReportComponent implements OnInit, OnDestroy {

    eventsOverTime: any[] = [];

    sourceRows: any[];
    destinationRows: any[];
    signatureRows: any[];
    categoryRows: any[];

    srcPorts: any[];
    destPorts: any[];

    loading = 0;

    queryString = "";

    private charts = {
        eventsOverTime: null,
    };

    subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    constructor(private appService: AppService,
                private ss: EveboxSubscriptionService,
                private route: ActivatedRoute,
                private reports: ReportsService,
                private api: ApiService,
                private topNavService: TopNavService,
                private elasticSearch: ElasticSearchService) {
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
        if (this.charts.eventsOverTime) {
            this.charts.eventsOverTime.destroy();
        }
        this.subTracker.unsubscribe();
    }

    load(fn: any): void {
        this.loading++;
        fn().then(() => {
        }).catch((err) => {
        }).then(() => {
            this.loading--;
        });
    }

    refresh(): void {

        const size = 10;

        this.sourceRows = undefined;
        this.destinationRows = undefined;
        this.signatureRows = undefined;

        const timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        const aggOptions: ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "alert",
        };

        this.load(() => {
            return this.api.reportAgg("alert.signature", aggOptions)
                .then((response: any) => {
                    this.signatureRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("alert.category", aggOptions)
                .then((response: any) => {
                    this.categoryRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response: any) => {
                    this.sourceRows = response.data;

                    this.sourceRows.forEach((row: any) => {
                        this.elasticSearch.resolveHostnameForIp(row.key).then((hostname: string) => {
                            if (hostname) {
                                row.searchKey = row.key;
                                row.key = `${row.key} (${hostname})`;
                            }
                        });
                    });
                });
        });

        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response: any) => {
                    this.destinationRows = response.data;

                    this.destinationRows.forEach((row: any) => {
                        this.elasticSearch.resolveHostnameForIp(row.key).then((hostname: string) => {
                            if (hostname) {
                                row.searchKey = row.key;
                                row.key = `${row.key} (${hostname})`;
                            }
                        });
                    });

                });
        });

        this.load(() => {
            return this.api.reportAgg("src_port", aggOptions)
                .then((response: any) => {
                    this.srcPorts = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dest_port", aggOptions)
                .then((response: any) => {
                    this.destPorts = response.data;
                });
        });

        this.load(() => {
            return this.api.reportHistogram({
                timeRange: timeRangeSeconds,
                interval: this.reports.histogramTimeInterval(timeRangeSeconds),
                eventType: "alert",
                queryString: this.queryString,
            }).then((response: any) => {
                const dataValues = [];
                const dataLabels = [];
                response.data.forEach((e: any) => {
                    dataValues.push(e.count);
                    dataLabels.push(moment(e.key).toDate());
                });
                const ctx = getCanvasElementById("eventsOverTimeChart");
                const config: ChartConfiguration = {
                    type: "bar",
                    data: {
                        labels: dataLabels,
                        datasets: [{
                            data: dataValues,
                            backgroundColor: getColourPalette(dataValues.length),
                        }]
                    },
                    options: {
                        plugins: {
                            title: {
                                display: true,
                                text: "Alerts Over Time",
                                padding: 0,
                            },
                            legend: {
                                display: false,
                            },
                        },
                        scales: {
                            x: {
                                type: "time",
                            }
                        }
                    },
                };
                if (this.charts.eventsOverTime) {
                    this.charts.eventsOverTime.destroy();
                }
                this.charts.eventsOverTime = new Chart(ctx, config);
            });
        });
    }
}
