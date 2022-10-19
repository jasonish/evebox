// Copyright (C) 2021 Jason Ish
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
import { ClientService } from "../../client.service";
import { getCanvasElementById } from "../../shared/chartjs";
import { Chart } from "chart.js";
import * as moment from "moment";
import { HttpParams } from "@angular/common/http";
import { TopNavService } from "../../topnav.service";
import { Subscription } from "rxjs";
import { AppEventCode, AppService } from "../../app.service";

@Component({
    selector: "app-stats",
    templateUrl: "./stats.component.html",
    styleUrls: ["./stats.component.scss"],
})
export class StatsComponent implements OnInit, OnDestroy {
    private charts: { [key: string]: any } = {};
    sensors = [];
    sensorName = "";
    subscriptions = new Subscription();
    totals: { [key: string]: number } = {};

    constructor(
        private client: ClientService,
        private topNav: TopNavService,
        private appService: AppService
    ) {}

    ngOnInit(): void {
        this.refresh();
        this.subscriptions.add(this.subscribeAppEvents());
    }

    ngOnDestroy(): void {
        this.subscriptions.unsubscribe();
        this.destroyCharts();
    }

    private subscribeAppEvents(): Subscription {
        return this.appService.subscribe((event: any) => {
            switch (event.event) {
                case AppEventCode.TIME_RANGE_CHANGED:
                    this.refresh();
                    break;
                case AppEventCode.IDLE:
                    break;
            }
        });
    }

    private destroyCharts(): void {
        for (const chart in this.charts) {
            if (this.charts[chart]) {
                this.charts[chart].destroy();
            }
        }
    }

    refresh(): void {
        this.refreshSensors();

        this.destroyCharts();

        this.refreshAgg(
            "chart-tcp-memuse",
            "stats.tcp.memuse",
            "TCP Memory Usage"
        );
        this.refreshAggDeriv(
            "chart-decoder-packets",
            "stats.decoder.pkts",
            "Decoder Packets"
        );
        this.refreshAggDeriv(
            "chart-decoder-bytes",
            "stats.decoder.bytes",
            "Decoder Bytes"
        );
        this.refreshAggDeriv(
            "chart-kernel-drops",
            "stats.capture.kernel_drops",
            "Kernel Drops"
        );
        this.refreshAgg(
            "chart-flow-memuse",
            "stats.flow.memuse",
            "Flow Memory Usage"
        );
    }

    private refreshSensors(): void {
        this.client.get("/api/1/sensors").subscribe((response) => {
            this.sensors = response.data;
        });
    }

    private getParams(): HttpParams {
        let params = new HttpParams().set(
            "time_range",
            this.topNav.getTimeRangeAsSeconds()
        );
        if (this.sensorName !== "") {
            params = params.set("sensor_name", this.sensorName);
        }
        return params;
    }

    private refreshAggDeriv(
        elementId: string,
        field: string,
        title: string
    ): void {
        const params = this.getParams().set("field", field);
        this.client
            .get("/api/1/stats/agg/deriv", params)
            .subscribe((response) => {
                const labels = [];
                const values = [];
                let total = 0;
                response.data.forEach((e) => {
                    labels.push(moment(e.timestamp).toDate());
                    values.push(e.value);
                    total += e.value;
                });
                this.totals[elementId] = total;
                this.charts[elementId] = this.buildChart(
                    elementId,
                    title,
                    labels,
                    values
                );
            });
    }

    private refreshAgg(elementId: string, field: string, title: string): void {
        const params = this.getParams().set("field", field);
        this.client.get("/api/1/stats/agg", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts[elementId] = this.buildChart(
                elementId,
                title,
                labels,
                values
            );
        });
    }

    private buildChart(
        elementId: string,
        title: string,
        labels: Date[],
        values: number[]
    ): Chart<any> {
        const ctx = getCanvasElementById(elementId);
        let min = null;
        if (Math.max(...values) === 0) {
            min = 0;
        }

        const chart = new Chart(ctx, {
            type: "line",
            data: {
                labels: labels,
                datasets: [
                    {
                        data: values,
                        backgroundColor: "rgba(0, 90, 0, 0.3)",
                        borderColor: "rgba(0, 90, 0, 1)",
                        pointRadius: 0,
                        fill: true,
                        borderWidth: 1,
                    },
                ],
            },
            options: {
                interaction: {
                    intersect: false,
                },
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        type: "time",
                    },
                    y: {
                        min: min,
                    },
                },
                plugins: {
                    title: {
                        text: title,
                        display: true,
                    },
                    legend: {
                        display: false,
                    },
                },
            },
        });
        return chart;
    }
}
