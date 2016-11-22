/* Copyright (c) 2014-2016 Jason Ish
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
import {Component, OnInit, OnDestroy} from "@angular/core";
import {ReportsService} from "./reports.service";
import {AppService, AppEventCode} from "../app.service";
import {EveboxFormatIpAddressPipe} from "../pipes/format-ipaddress.pipe";
import {ActivatedRoute, Params} from "@angular/router";
import {EveboxSubscriptionService} from "../subscription.service";
import {loadingAnimation} from "../animations";
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ApiService, QueryStringBuilder, ReportAggOptions} from "../api.service";
import {TopNavService} from "../topnav.service";
import moment = require("moment");

@Component({
    template: `<div [@loadingState]="(loading > 0) ? 'true' : 'false'">

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

  <metrics-graphic *ngIf="eventsOverTime"
                   graphId="alertsOverTimeGraph"
                   title="Alerts Over Time"
                   [data]="eventsOverTime"></metrics-graphic>

  <div class="row">
    <div class="col-md-6">
      <report-data-table *ngIf="signatureRows"
                         title="Top Alert Signatures"
                         [rows]="signatureRows"
                         [headers]="['#', 'Signature']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table *ngIf="categoryRows"
                         title="Top Alert Categories"
                         [rows]="categoryRows"
                         [headers]="['#', 'Category']"></report-data-table>
    </div>
  </div>

  <div class="row">
    <div class="col-md-6">
      <report-data-table *ngIf="sourceRows"
                         title="Top Alerting Source IPs"
                         [rows]="sourceRows"
                         [headers]="['#', 'Source']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table *ngIf="destinationRows"
                         title="Top Alerting Destination IPs"
                         [rows]="destinationRows"
                         [headers]="['#', 'Destination']"></report-data-table>
    </div>
  </div>

  <div class="row">
    <div class="col-md-6">
      <report-data-table *ngIf="srcPorts"
                         title="Top Alerting Source Ports"
                         [rows]="srcPorts"
                         [headers]="['#', 'Port']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table *ngIf="destPorts"
                         title="Top Alerting Destination Ports"
                         [rows]="destPorts"
                         [headers]="['#', 'Port']"></report-data-table>
    </div>
  </div>

</div>`,
    animations: [
        loadingAnimation,
    ]
})
export class AlertReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[] = [];

    private sourceRows:any[];
    private destinationRows:any[];
    private signatureRows:any[];
    private categoryRows:any[];

    private srcPorts:any[];
    private destPorts:any[];

    private loading:number = 0;

    private queryString:string = "";

    private subTracker:EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

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

        let size:number = 10;

        this.sourceRows = undefined;
        this.destinationRows = undefined;
        this.signatureRows = undefined;

        let timeRangeSeconds = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions:ReportAggOptions = {
            queryString: this.queryString,
            timeRange: timeRangeSeconds,
            size: size,
            eventType: "alert",
        };

        this.load(() => {
            return this.api.reportAgg("alert.signature", aggOptions)
                .then((response:any) => {
                    this.signatureRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("alert.category", aggOptions)
                .then((response:any) => {
                    this.categoryRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response:any) => {
                    this.sourceRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response:any) => {
                    this.destinationRows = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("src_port", aggOptions)
                .then((response:any) => {
                    this.srcPorts = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dest_port", aggOptions)
                .then((response:any) => {
                    this.destPorts = response.data;
                });
        });

        this.load(() => {
            return this.api.reportHistogram({
                timeRange: timeRangeSeconds,
                interval: this.reports.histogramTimeInterval(timeRangeSeconds),
                eventType: "alert",
                queryString: this.queryString,
            }).then((response:any) => {
                this.eventsOverTime = response.data.map((x:any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.count,
                    }
                })
            })
        });

    }

}
