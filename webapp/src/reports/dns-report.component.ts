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

import {Component, OnDestroy, OnInit, Input} from "@angular/core";
import {Router} from "@angular/router";

import {ReportsService} from "../reports.service";
import {EveboxMetricsGraphicComponent} from "../metricgraphics.component";
import {AppService, AppEventCode} from "../app.service";
import {EveboxSearchLinkComponent} from "../search-link.component";
import {ToastrService} from "../toastr.service";

import moment = require("moment");

@Component({
    selector: "report-data-table",
    template: `<div class="panel panel-default">
  <div class="panel-heading">
    <b>{{title}}</b>
  </div>
  <div *ngIf="!rows">
    <div class="panel-body" style="text-align: center;">
      <i class="fa fa-spinner fa-pulse"
         style="font-size: 200px; opacity: 0.5;"></i>
    </div>
  </div>

  <table class="table table-striped table-condensed">
    <thead>
    <tr>
      <th *ngFor="let header of headers">{{header}}</th>
    </tr>
    </thead>
    <tbody>
    <tr *ngFor="let row of rows; let i = index">
      <td>{{row.count}}</td>
      <td><a href='#/events?q="{{row.key}}"'>{{row.key}}</a></td>
    </tr>
    </tbody>
  </table>
</div>`,
    directives: [
        EveboxSearchLinkComponent
    ]
})
export class EveboxReportDataTable {

    @Input() private title:string;
    @Input() private headers:string[] = [];

    @Input() private rows:any[];
    @Input() private searchField:string;

}

@Component({
    template: `<div>

  <metrics-graphic graphId="dnsRequestsOverTime"
                   title="DNS Requests Over Time"
                   [data]="eventsOverTime"></metrics-graphic>
  
  <div class="row">
    <div class="col-md-6">
      <report-data-table title="Top Request RRNames"
                         [rows]="topRrnames"
                         [headers]="['#', 'RRName']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table title="Top Response Rdata"
                         [rows]="topRdata"
                         [headers]="['#', 'Rdata']"></report-data-table>
    </div>
  </div>

  <div class="row">

    <div class="col-md-6">

      <report-data-table title="Top DNS Servers"
                         [rows]="topServers"
                         [headers]="['#', 'Server']"></report-data-table>

    </div>

    <div class="col-md-6">
      <report-data-table title="Top DNS Clients"
                         [rows]="topClients"
                         [headers]="['#', 'Client']"></report-data-table>
    </div>

  </div>

  <div class="row">
    <div class="col-md-6">
      <report-data-table title="Top Requests Types"
                         [rows]="topRrtypes"
                         [headers]="['#', 'RRType']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table title="Top Response Codes"
                         [rows]="topRcodes"
                         [headers]="['#', 'RCode']"></report-data-table>
    </div>
  </div>

</div>`,
    directives: [
        EveboxMetricsGraphicComponent,
        EveboxReportDataTable
    ]
})
export class DNSReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[] = [];

    private topRrnames:any[] = [];
    private topRdata:any[] = [];
    private topRrtypes:any[] = [];
    private topRcodes:any[] = [];
    private topServers:any[] = [];
    private topClients:any[] = [];

    private dispatcherSubscription:any;
    private destroyed:boolean = false;

    constructor(private reports:ReportsService,
                private appService:AppService,
                private toastr:ToastrService) {
    }

    ngOnInit() {

        this.toastr.warning("Reports are experimental are are subject to change.", {
            title: "Warning",
            closeButton: true
        });

        this.refresh();

        this.dispatcherSubscription = this.appService.subscribe((event:any) => {
            if (this.destroyed) {
                return;
            }
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.destroyed = true;
        this.dispatcherSubscription.unsubscribe();
    }

    refresh() {

        this.reports.dnsResponseReport().then((response:any) => {

            this.topRdata = response.aggregations.top_rdata.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

            this.topRcodes = response.aggregations.top_rcode.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

        });

        this.reports.dnsRequestReport().then((response:any) => {

            this.eventsOverTime = response.aggregations.events_over_time.buckets.map((item:any) => {
                return {
                    date: moment(item.key).toDate(),
                    value: item.doc_count
                }
            });

            this.topRrnames = response.aggregations.top_rrnames.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

            this.topServers = response.aggregations.top_servers.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

            this.topClients = response.aggregations.top_clients.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

            this.topRrtypes = response.aggregations.top_rrtype.buckets.map((item:any) => {
                return {
                    key: item.key,
                    count: item.doc_count
                };
            });

        });

    }
}