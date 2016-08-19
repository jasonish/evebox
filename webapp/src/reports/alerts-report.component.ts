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
import {ROUTER_DIRECTIVES} from "@angular/router";
import {ReportsService} from "./reports.service";
import {EveboxSearchLinkComponent} from "../search-link.component";
import {AppService, AppEventCode} from "../app.service";
import {EveboxMapToItemsPipe} from "../pipes/maptoitems.pipe";
import {EveboxReportDataTable} from "./dns-report.component";
import {EveboxMetricsGraphicComponent} from "../metricgraphics.component";

import moment = require("moment");
import {ToastrService} from "../toastr.service";
import {EveboxLoadingSpinnerComponent} from "../loading-spinner.component";
import {EveboxFormatIpAddressPipe} from "../pipes/format-ipaddress.pipe";

@Component({
    template: `<div [ngClass]="{'evebox-opacity-50': loading > 0}">

  <loading-spinner [loading]="loading > 0"></loading-spinner>

  <div class="row">
    <div class="col-md-12">
      <button type="button" class="btn btn-default" (click)="refresh()">Refresh
      </button>
    </div>
  </div>

  <br/>

  <metrics-graphic *ngIf="alertsOverTime"
                   graphId="alertsOverTimeGraph"
                   title="Alerts Over Time"
                   [data]="alertsOverTime"></metrics-graphic>

  <div class="row">
    <div class="col-md-12">

      <report-data-table *ngIf="signatureRows"
                         title="Top Alert Signatures"
                         [rows]="signatureRows"
                         [headers]="['#', 'Signature']"></report-data-table>

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

</div>`,
    directives: [
        EveboxSearchLinkComponent,
        EveboxReportDataTable,
        EveboxMetricsGraphicComponent,
        EveboxLoadingSpinnerComponent,
        ROUTER_DIRECTIVES
    ],
    pipes: [
        EveboxMapToItemsPipe
    ]
})
export class AlertReportComponent implements OnInit, OnDestroy {

    private alertsOverTime:any[] = [];

    private sourceRows:any[];
    private destinationRows:any[];
    private signatureRows:any[];

    private dispatcherSubscription:any;

    private loading:number = 0;

    constructor(private appService:AppService,
                private reports:ReportsService,
                private formatIpAddressPipe:EveboxFormatIpAddressPipe) {
    }

    ngOnInit() {

        this.reports.showWarning();

        this.refresh();

        this.dispatcherSubscription = this.appService.subscribe((event:any) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.dispatcherSubscription.unsubscribe();
    }

    mapAggregation(response:any, name:string):any[] {
        return response.aggregations[name].buckets.map((item:any) => {
            return {
                count: item.doc_count,
                key: item.key
            }
        });
    }

    refresh() {

        let size:number = 20;

        this.loading++;

        this.sourceRows = undefined;
        this.destinationRows = undefined;
        this.signatureRows = undefined;

        this.reports.alertsReport({size: size}).then(
            (response:any) => {

                this.sourceRows = this.mapAggregation(response, "sources")
                    .map((row:any) => {
                        return {
                            count: row.count,
                            key: this.formatIpAddressPipe.transform(row.key)
                        }
                    });

                this.destinationRows = this.mapAggregation(response, "destinations")/**/
                    .map((row:any) => {
                        return {
                            count: row.count,
                            key: this.formatIpAddressPipe.transform(row.key)
                        }
                    });

                this.signatureRows = this.mapAggregation(response, "signatures");

                this.alertsOverTime = response.aggregations.alerts_per_minute.buckets.map((x:any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.doc_count
                    }
                });

                this.loading--;

            });

    }
}