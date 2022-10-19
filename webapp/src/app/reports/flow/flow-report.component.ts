/* Copyright (c) 2016-2021 Jason Ish
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

import { Component, HostListener, OnDestroy, OnInit } from "@angular/core";
import { ActivatedRoute, Params } from "@angular/router";
import { ReportsService } from "../reports.service";
import { AppEvent, AppEventCode, AppService } from "../../app.service";
import { TopNavService } from "../../topnav.service";
import { ElasticSearchService } from "../../elasticsearch.service";
import { loadingAnimation } from "../../animations";
import { EveboxSubscriptionTracker } from "../../subscription-tracker";
import { ApiService, ReportAggOptions } from "../../api.service";
import { EveBoxProtoPrettyPrinter } from "../../pipes/proto-pretty-printer.pipe";

import * as chartjs from "../../shared/chartjs";
import * as moment from "moment";
import { finalize } from "rxjs/operators";
import { Observable } from "rxjs";

import { Chart } from "chart.js";

@Component({
    templateUrl: "flow-report.component.html",
    animations: [loadingAnimation],
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
        { value: "1s", msg: "1 second" },
        { value: "1m", msg: "1 minute" },
        { value: "5m", msg: "5 minute" },
        { value: "15m", msg: "15 minutes" },
        { value: "1h", msg: "1 hour" },
        { value: "6h", msg: "6 hours" },
        { value: "12h", msg: "12 hours" },
        { value: "1d", msg: "1 day (24 hours)" },
    ];

    public interval: string = null;

    private range: number = null;

    // A boolean to toggle to remove that chart elements and re-add them
    // to fix issues with ChartJS resizing.
    showCharts = true;

    constructor(
        private appService: AppService,
        private route: ActivatedRoute,
        private reportsService: ReportsService,
        private topNavService: TopNavService,
        private api: ApiService,
        private elasticsearch: ElasticSearchService,
        private protoPrettyPrinter: EveBoxProtoPrettyPrinter
    ) {}

    ngOnInit(): void {
        this.range = this.topNavService.getTimeRangeAsSeconds();

        this.subTracker.subscribe(this.route.queryParams, (params: Params) => {
            this.queryString = params.q || "";
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

    ngOnDestroy(): void {
        this.subTracker.unsubscribe();
    }

    load(fn: any): void {
        this.loading++;
        fn()
            .then(() => {})
            .catch((_) => {})
            .then(() => {
                this.loading--;
            });
    }

    private renderChart(id: string, options: any): void {
        const ctx = chartjs.getCanvasElementById(id);
        if (this.charts[id]) {
            this.charts[id].chart.destroy();
        }
        this.charts[id] = {
            chart: new Chart(ctx, options),
            id: id,
            options: options,
        };
    }

    @HostListener("window:resize", ["$event"])
    private onResize(event): void {
        this.showCharts = false;
        setTimeout(() => {
            this.showCharts = true;
            setTimeout(() => {
                for (const key of Object.keys(this.charts)) {
                    this.renderChart(
                        this.charts[key].id,
                        this.charts[key].options
                    );
                }
            });
        }, 0);
    }

    private refreshEventsOverTime(): void {
        let displayUnit = "minute";

        if (!this.interval) {
            this.interval = "1d";

            if (this.range === 0) {
                displayUnit = "day";
            } else if (this.range <= 60) {
                this.interval = "1s";
                displayUnit = "second";
            } else if (this.range <= 3600) {
                this.interval = "1m";
                displayUnit = "minute";
            } else if (this.range <= 3600 * 3) {
                this.interval = "5m";
                displayUnit = "minute";
            } else if (this.range <= 3600 * 24) {
                this.interval = "15m";
                displayUnit = "hour";
            } else if (this.range <= 3600 * 24 * 3) {
                this.interval = "1h";
                displayUnit = "hour";
            } else if (this.range <= 3600 * 24 * 7) {
                this.interval = "1h";
                displayUnit = "hour";
            }
        }

        console.log(`interval: ${this.interval}, displayUnit: ${displayUnit}`);

        const histogramOptions: any = {
            appProto: true,
            queryString: this.queryString,
            interval: this.interval,
        };

        if (this.range > 0) {
            histogramOptions.timeRange = `${this.range}s`;
        }

        this.wrap(this.api.flowHistogram(histogramOptions)).subscribe(
            (response) => {
                const labels = [];
                const eventCounts = [];
                const protos = [];

                response.data.forEach((elem) => {
                    for (const proto in elem.app_proto) {
                        if (protos.indexOf(proto) < 0) {
                            protos.push(proto);
                        }
                    }
                });

                const data = {};

                const colours = chartjs.getColourPalette(protos.length + 1);

                const totals = [];

                response.data.forEach((elem) => {
                    let protoSum = 0;
                    for (const proto of protos) {
                        if (!data[proto]) {
                            data[proto] = [];
                        }
                        if (proto in elem.app_proto) {
                            const val = elem.app_proto[proto];
                            data[proto].push(val);
                            protoSum += val;
                        } else {
                            data[proto].push(0);
                        }
                    }
                    labels.push(moment(elem.key).toDate());

                    totals.push(elem.events);
                    eventCounts.push(elem.events - protoSum);
                });

                const datasets: any[] = [
                    {
                        label: "Other",
                        backgroundColor: colours[0],
                        borderColor: colours[0],
                        data: eventCounts,
                        fill: false,
                    },
                ];

                let i = 1;

                for (const proto of protos) {
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

                const chartOptions = {
                    type: "bar",
                    data: {
                        labels: labels,
                        datasets: datasets,
                    },
                    options: {
                        plugins: {
                            legend: {
                                position: "right",
                            },
                            title: {
                                display: true,
                                text: "Flow Events Over Time",
                                padding: 0,
                            },
                        },
                        title: {
                            display: true,
                            text: "Flow Events Over Time",
                        },
                        scales: {
                            x: {
                                display: true,
                                type: "time",
                                stacked: true,
                            },
                            y: {
                                stacked: true,
                                grid: {
                                    display: false,
                                },
                                ticks: {
                                    padding: 5,
                                },
                            },
                        },
                        maintainAspectRatio: false,
                        responsive: true,
                    },
                };

                this.renderChart("eventsOverTimeChart", chartOptions);
            }
        );
    }

    changeHistogramInterval(interval): void {
        this.interval = interval;
        this.refreshEventsOverTime();
    }

    refresh(): void {
        this.refreshEventsOverTime();

        const aggOptions: ReportAggOptions = {
            timeRange: this.range,
            eventType: "flow",
            size: 10,
            queryString: this.queryString,
        };

        this.load(() => {
            return this.api
                .reportAgg("src_ip", aggOptions)
                .then((response: any) => {
                    this.topClientsByFlows = response.data;
                });
        });

        this.load(() => {
            return this.api
                .reportAgg("dest_ip", aggOptions)
                .then((response: any) => {
                    this.topServersByFlows = response.data;
                });
        });

        this.load(() => {
            return this.api
                .reportAgg("traffic.id", aggOptions)
                .then((response: any) => {
                    const labels = response.data.map((e) => e.key);
                    const data = response.data.map((e) => e.count);

                    if (response.missing && response.missing > 0) {
                        labels.push("<no-id>");
                        data.push(response.missing);
                    }

                    if (response.other && response.other > 0) {
                        labels.push("<other>");
                        data.push(response.other);
                    }

                    const colours = chartjs.getColourPalette(labels.length + 1);

                    const config = {
                        type: "pie",
                        data: {
                            datasets: [
                                {
                                    data: data,
                                    backgroundColor: colours,
                                },
                            ],
                            labels: labels,
                        },
                        options: {
                            responsive: true,
                        },
                    };
                    this.renderChart("trafficIdChart", config);
                });
        });

        this.load(() => {
            return this.api
                .reportAgg("traffic.label", aggOptions)
                .then((response: any) => {
                    const labels = response.data.map((e) => e.key);
                    const data = response.data.map((e) => e.count);

                    if (response.missing && response.missing > 0) {
                        labels.push("<unlabeled>");
                        data.push(response.missing);
                    }

                    if (response.other && response.other > 0) {
                        labels.push("<other>");
                        data.push(response.other);
                    }

                    const colours = chartjs.getColourPalette(labels.length + 1);

                    const options = {
                        type: "pie",
                        data: {
                            datasets: [
                                {
                                    data: data,
                                    backgroundColor: colours,
                                },
                            ],
                            labels: labels,
                        },
                    };
                    this.renderChart("trafficLabelChart", options);
                });
        });

        this.wrap(
            this.api.eventQuery({
                queryString: this.queryString,
                eventType: "flow",
                size: 10,
                timeRange: this.range,
                sortBy: "flow.age",
                sortOrder: "desc",
            })
        ).subscribe((response) => {
            this.topFlowsByAge = response.data;
        });
    }

    private wrap(observable: Observable<any>): Observable<any> {
        this.loading++;
        return observable.pipe(
            finalize(() => {
                this.loading--;
            })
        );
    }
}
