import { Component, OnDestroy, OnInit } from '@angular/core';
import { ClientService } from "../../client.service";
import { getCanvasElementById, getColourPalette } from "../../shared/chartjs";
import { Chart } from "chart.js";
import * as moment from "moment";
import { HttpParams } from "@angular/common/http";
import { TopNavService } from "../../topnav.service";
import { Subscription } from "rxjs";
import { AppEventCode, AppService } from "../../app.service";

@Component({
    selector: 'app-stats',
    templateUrl: './stats.component.html',
    styleUrls: ['./stats.component.scss']
})
export class StatsComponent implements OnInit, OnDestroy {

    private charts = {
        decoderBytes: null,
        flowMemUse: null,
        tcpMemuse: null,
        kernelPackets: null,
        kernelDrops: null,
    };

    sensors = [];
    sensorName = "";

    subscriptions = new Subscription();

    constructor(private client: ClientService, private topNav: TopNavService, private appService: AppService) {
    }

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
            if (this.charts[chart] != null) {
                this.charts[chart].destroy();
            }
        }
    }

    refresh(): void {
        this.refreshSensors();

        this.destroyCharts();
        this.refreshTcpMemuse();
        this.refreshFlowMemuse();
        this.refreshKernelPackets();
        this.refreshKernelDrops();
        this.refreshDecoderBytes();
    }

    private refreshSensors(): void {
        this.client.get("/api/1/sensors").subscribe((response) => {
            this.sensors = response.data;
        });
    }

    private getParams(): HttpParams {
        let params = new HttpParams().set("time_range", this.topNav.getTimeRangeAsSeconds());
        if (this.sensorName !== "") {
            params = params.set("sensor_name", this.sensorName);
        }
        return params;
    }

    private refreshDecoderBytes(): void {
        const params = this.getParams().set("field", "stats.decoder.bytes");
        this.client.get("/api/1/stats/agg/deriv", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts.decoderBytes = this.buildChart("chart-decoder-bytes", "Decoder Bytes", labels, values);
        });
    }

    private refreshKernelPackets(): void {
        const params = this.getParams().set("field", "stats.capture.kernel_packets");
        this.client.get("/api/1/stats/agg/deriv", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts.kernelPackets = this.buildChart("chart-kernel-packets", "Kernel Packets", labels, values);
        });
    }

    private refreshKernelDrops(): void {
        const params = this.getParams().set("field", "stats.capture.kernel_drops");
        this.client.get("/api/1/stats/agg/deriv", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts.kernelDrops = this.buildChart("chart-kernel-drops", "Kernel Drops", labels, values);
        });
    }

    private refreshTcpMemuse(): void {
        const params = this.getParams().set("field", "stats.tcp.memuse");
        this.client.get("/api/1/stats/agg", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts.tcpMemuse = this.buildChart("chart-tcp-memuse", "TCP Memory Usage", labels, values);
        });
    }

    private refreshFlowMemuse(): void {
        const params = this.getParams().set("field", "stats.flow.memuse");
        this.client.get("/api/1/stats/agg", params).subscribe((response) => {
            const labels = [];
            const values = [];
            response.data.forEach((e) => {
                labels.push(moment(e.timestamp).toDate());
                values.push(e.value);
            });
            this.charts.flowMemUse = this.buildChart("chart-flow-memuse", "Flow Memory Usage", labels, values);
        });
    }

    private buildChart(elementId: string, title: string, labels: Date[], values: number[]): Chart<any> {
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
                    }
                ]
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
                    }
                },
                plugins: {
                    title: {
                        text: title,
                        display: true,
                    },
                    legend: {
                        display: false,
                    }
                }
            }
        });
        return chart;
    }
}
