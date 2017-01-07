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

import {Component, OnInit, OnDestroy, Input, OnChanges} from "@angular/core";
import {ReportsService} from "./reports.service";
import {AppService, AppEventCode} from "../app.service";
import {EveboxFormatIpAddressPipe} from "../pipes/format-ipaddress.pipe";
import {ActivatedRoute, Params} from "@angular/router";
import {EveboxSubscriptionService} from "../subscription.service";
import {loadingAnimation} from "../animations";
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ApiService, ReportAggOptions} from "../api.service";
import {TopNavService} from "../topnav.service";
import * as moment from "moment";

require("chart.js");
declare var Chart:any;

@Component({
    selector: "evebox-ssh-top-client-hosts",
    template: `<report-data-table *ngIf="results"
                   title="Top SSH Client Hosts"
                   [rows]="results"
                   [headers]="['#', 'Address']"></report-data-table>
`,
})
export class SshTopClientsComponent implements OnInit, OnChanges {

    @Input() queryString:string = "";

    private results:any[] = [];

    constructor(private api:ApiService, private topNavService:TopNavService) {
    }

    ngOnInit():void {
        this.refresh();
    }

    ngOnChanges():void {
        this.refresh();
    }

    refresh():any {

        let size:number = 10;

        let timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions:ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "ssh",
        };

        return this.api.reportAgg("src_ip", aggOptions)
            .then((response:any) => {
                this.results = response.data;
            });
    }
}

@Component({
    selector: "evebox-ssh-top-server-hosts",
    template: `<report-data-table *ngIf="results"
                   title="Top SSH Server Hosts"
                   [rows]="results"
                   [headers]="['#', 'Address']"></report-data-table>
`,
})
export class SshTopServersComponent implements OnInit, OnChanges {

    @Input() queryString:string = "";

    private results:any[] = [];

    constructor(private api:ApiService, private topNavService:TopNavService) {
    }

    ngOnInit():void {
        this.refresh();
    }

    ngOnChanges():void {
        this.refresh();
    }

    refresh():any {

        let size:number = 10;

        let timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions:ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "ssh",
        };

        return this.api.reportAgg("dest_ip", aggOptions)
            .then((response:any) => {
                this.results = response.data;
            });
    }
}

@Component({
    template: `<div class="content" [@loadingState]="(loading > 0) ? 'true' : 'false'">

  <loading-spinner [loading]="loading > 0"></loading-spinner>

  <div class="row">
    <div class="col-md-6 col-sm-6">
      <button type="button" class="btn btn-default" (click)="refresh()">
        Refresh
      </button>
    </div>
    <div class="col-md-6 col-sm-6">
      <evebox-filter-input [queryString]="queryString"></evebox-filter-input>
    </div>
  </div>

  <br/>

  <div class="row">
    <div class="col-md-12">
      <canvas id="eventsOverTimeChart" height="225"></canvas>
    </div>
  </div>

  <br/>

  <div class="row">
    <div class="col-md-6">
      <div class="panel panel-default">
        <div class="panel-heading">
          SSH Client Software
        </div>
        <div class="panel-body">
          <canvas id="clientVersionsPie" style="height: 300px;"></canvas>
        </div>
      </div>
    </div>
    <div class="col-md-6">
      <div class="panel panel-default">
        <div class="panel-heading">
          SSH Server Software
        </div>
        <div class="panel-body">
          <canvas id="serverVersionsPie" style="height: 300px;"></canvas>
        </div>
      </div>
    </div>
  </div>

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

  <div class="row">
    <div class="col-md-6">
      <evebox-ssh-top-client-hosts [queryString]="queryString"></evebox-ssh-top-client-hosts>
    </div>
    <div class="col-md-6">
      <evebox-ssh-top-server-hosts [queryString]="queryString"></evebox-ssh-top-server-hosts>
    </div>
  </div>

</div>`,
    animations: [
        loadingAnimation,
    ]
})
export class SshReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[] = [];

    private serverSoftware:any[] = [];
    private clientSoftware:any[] = [];

    private loading:number = 0;

    private queryString:string = "";

    private subTracker:EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    private charts:any = {};

    constructor(private appService:AppService,
                private ss:EveboxSubscriptionService,
                private route:ActivatedRoute,
                private reports:ReportsService,
                private api:ApiService,
                private topNavService:TopNavService,
                private formatIpAddressPipe:EveboxFormatIpAddressPipe) {
    }

    ngOnInit() {

        if (this.route.snapshot.queryParams["q"]) {
            this.queryString = this.route.snapshot.queryParams["q"];
        }

        this.subTracker.subscribe(this.route.params, (params:Params) => {
            this.queryString = params["q"] || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event:any) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.subTracker.unsubscribe();
    }

    load(fn:any) {
        this.loading++;
        fn().then(() => {
            this.loading--;
        })
    }

    refresh() {

        let size:number = 100;

        let timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions:ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "ssh",
        };

        this.load(() => {
            return this.api.reportHistogram({
                timeRange: timeRangeSeconds,
                interval: this.reports.histogramTimeInterval(timeRangeSeconds),
                eventType: "ssh",
                queryString: this.queryString,
            }).then((response:any) => {
                this.eventsOverTime = response.data.map((x:any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.count,
                    }
                });

                let ctx = document.getElementById("eventsOverTimeChart");

                let values:any[] = response.data.map((x:any) => {
                    return x.count;
                });

                let labels:any[] = response.data.map((x:any) => {
                    return moment(x.key).format();
                });

                if (this.charts["eventsOverTimeChart"]) {
                    this.charts["eventsOverTimeChart"].destroy();
                }

                this.charts["eventsOverTimeChart"] = new Chart(ctx, {
                    type: "line",
                    data: {
                        labels: labels,
                        datasets: [
                            {
                                backgroundColor: randomColour(),
                                data: values,
                                pointStyle: "line",
                            }
                        ]
                    },
                    options: {
                        title: {
                            display: true,
                            text: "SSH Connections Over Time",
                        },
                        scales: {
                            xAxes: [
                                {
                                    type: "time",
                                    ticks: {
                                        maxRotation: 0,
                                    }
                                }
                            ]
                        },
                        legend: {
                            display: false,
                        },
                        maintainAspectRatio: false,
                    },
                })

            })
        });

        this.load(() => {
            return this.api.reportAgg("ssh.client.software_version", aggOptions)
                .then((response:any) => {

                    this.clientSoftware = response.data;

                    // Only graph the top 10 then sum up the rest under "Other".
                    let versions:any = [];

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

                    this.renderPieChart("clientVersionsPie", versions);
                })
        });

        this.load(() => {
            return this.api.reportAgg("ssh.server.software_version", aggOptions)
                .then((response:any) => {

                    this.serverSoftware = response.data;

                    // Only graph the top 10 then sum up the rest under "Other".
                    let versions:any = [];

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
                })
        });

    }

    renderPieChart(canvasId:string, data:any[]) {

        let labels:string[] = [];
        let values:number[] = [];
        let colours:string[] = [];

        data.forEach((version:any) => {
            labels.push(version.key);
            values.push(version.count);
            colours.push(randomColour());
        });

        let ctx = document.getElementById(canvasId);

        if (this.charts[canvasId]) {
            this.charts[canvasId].destroy();
        }

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
                }
            }
        });

    }

}

function randomColour() {
    let r = Math.floor(Math.random() * 255);
    let g = Math.floor(Math.random() * 255);
    let b = Math.floor(Math.random() * 255);
    let color = 'rgb(' + r + ', ' + g + ', ' + b + ')';
    return color;
}
