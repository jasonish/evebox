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
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ActivatedRoute, Params} from "@angular/router";
import {ApiService, ReportAggOptions} from "../api.service";
import {TopNavService} from "../topnav.service";

import * as moment from "moment";

@Component({
    template: `<div class="content" [ngClass]="{'evebox-opacity-50': loading > 0}">

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

    private loading:number = 0;

    private queryString:string = "";

    private subTracker:EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    constructor(private route:ActivatedRoute,
                private reports:ReportsService,
                private appService:AppService,
                private api:ApiService,
                private topNavService:TopNavService,
                private reportsService:ReportsService,
                private formatIpAddressPipe:EveboxFormatIpAddressPipe) {
    }

    ngOnInit() {

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


    load(fn:any) {
        this.loading++;
        fn().then(() => {
            this.loading--;
        })
    }

    refresh() {

        let size = 20;
        let range = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions:ReportAggOptions = {
            eventType: "dns",
            dnsType: "answer",
            timeRange: range,
            queryString: this.queryString,
            size: size,
        };


        this.load(() => {
            return this.api.reportAgg("dns.rcode", aggOptions)
                .then((response:any) => {
                    this.topRcodes = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dns.rdata", aggOptions)
                .then((response:any) => {
                    this.topRdata = response.data;
                });
        });

        // Switch to request queries.
        aggOptions.dnsType = "query";

        this.load(() => {
            return this.api.reportAgg("dns.rrname", aggOptions)
                .then((response:any) => {
                    this.topRrnames = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("dns.rrtype", aggOptions)
                .then((response:any) => {
                    this.topRrtypes = response.data;
                });
        });

        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response:any) => {
                    this.topClients = response.data;
                })
        });

        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response:any) => {
                    this.topServers = response.data;
                })
        });

        // Queries over time histogram.
        this.load(() => {
            return this.api.reportHistogram({
                timeRange: range,
                interval: this.reportsService.histogramTimeInterval(range),
                eventType: "dns",
                dnsType: "query",
                queryString: this.queryString,
            }).then((response:any) => {
                this.eventsOverTime = response.data.map((x:any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.count
                    }
                });
            });
        });

    }
}