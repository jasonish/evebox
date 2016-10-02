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

import {Component, OnDestroy, OnInit} from "@angular/core";
import {ReportsService} from "./reports.service";
import {AppService, AppEventCode} from "../app.service";
import {EveboxFormatIpAddressPipe} from "../pipes/format-ipaddress.pipe";

import moment = require("moment");

@Component({
    template: `<div [ngClass]="{'evebox-opacity-50': loading > 0}">

  <loading-spinner [loading]="loading > 0"></loading-spinner>

  <metrics-graphic *ngIf="eventsOverTime"
                   graphId="dnsRequestsOverTime"
                   title="DNS Requests Over Time"
                   [data]="eventsOverTime"></metrics-graphic>

  <div class="row">
    <div class="col-md-6">
      <report-data-table *ngIf="topRrnames"
                         title="Top Request RRNames"
                         [rows]="topRrnames"
                         [headers]="['#', 'RRName']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table *ngIf="topRdata"
                         title="Top Response Rdata"
                         [rows]="topRdata"
                         [headers]="['#', 'Rdata']"></report-data-table>
    </div>
  </div>

  <div class="row">

    <div class="col-md-6">
      <report-data-table *ngIf="topServers"
                         title="Top DNS Servers"
                         [rows]="topServers"
                         [headers]="['#', 'Server']"></report-data-table>
    </div>

    <div class="col-md-6">
      <report-data-table *ngIf="topClients"
                         title="Top DNS Clients"
                         [rows]="topClients"
                         [headers]="['#', 'Client']"></report-data-table>
    </div>

  </div>

  <div class="row">
    <div class="col-md-6">
      <report-data-table *ngIf="topRrtypes"
                         title="Top Requests Types"
                         [rows]="topRrtypes"
                         [headers]="['#', 'RRType']"></report-data-table>
    </div>
    <div class="col-md-6">
      <report-data-table *ngIf="topRcodes"
                         title="Top Response Codes"
                         [rows]="topRcodes"
                         [headers]="['#', 'RCode']"></report-data-table>
    </div>
  </div>

</div>`,
})
export class DNSReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[];

    private topRrnames:any[];
    private topRdata:any[];
    private topRrtypes:any[];
    private topRcodes:any[];
    private topServers:any[];
    private topClients:any[];

    private dispatcherSubscription:any;
    private destroyed:boolean = false;

    private loading:number = 0;

    constructor(private reports:ReportsService,
                private appService:AppService,
                private formatIpAddressPipe:EveboxFormatIpAddressPipe) {
    }

    ngOnInit() {

        this.reports.showWarning();

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

    mapAddressAggregation(items:any[]) {
        return items.map((item:any) => {

            let key = item.key;

            // If key looks like an IP address, format it.
            if (key.match(/\d*\.\d*\.\d*\.\d*/)) {
                key = this.formatIpAddressPipe.transform(key);
            }

            return {
                key: key,
                count: item.doc_count,
            }

        });
    }

    mapAggregation(items:any[]) {
        return items.map((item:any) => {
            return {
                key: item.key,
                count: item.doc_count,
            };
        });
    }

    refresh() {

        let size = 20;

        this.loading++;

        this.reports.dnsResponseReport({size: size}).then((response:any) => {

            this.topRdata = this.mapAddressAggregation(response.aggregations.top_rdata.buckets);
            this.topRcodes = this.mapAggregation(response.aggregations.top_rcode.buckets);

            this.loading--;

        });

        this.loading++;

        this.reports.dnsRequestReport({size: size}).then((response:any) => {

            this.eventsOverTime = response.aggregations.events_over_time.buckets.map((item:any) => {
                return {
                    date: moment(item.key).toDate(),
                    value: item.doc_count
                }
            });

            this.topRrnames = this.mapAggregation(response.aggregations.top_rrnames.buckets);
            this.topServers = this.mapAddressAggregation(response.aggregations.top_servers.buckets);
            this.topClients = this.mapAddressAggregation(response.aggregations.top_clients.buckets);
            this.topRrtypes = this.mapAggregation(response.aggregations.top_rrtype.buckets);

            this.loading--;

        });

    }
}