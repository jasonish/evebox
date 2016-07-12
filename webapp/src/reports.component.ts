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

import {Component, OnInit, Input, OnDestroy} from "@angular/core";
import {ReportsService} from "./reports.service";
import {EveboxSearchLinkComponent} from "./search-link.component";
import {ROUTER_DIRECTIVES, Router} from "@angular/router";
import {AppService, AppEventCode} from "./app.service";

import moment = require("moment");
let MG = require("metrics-graphics");
import "metrics-graphics/dist/metricsgraphics.css";

@Component({
    selector: "simple-report",
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
      <th></th>
      <th>#</th>
      <th>{{header}}</th>
    </tr>
    </thead>
    <tbody>
    <tr *ngFor="let row of rows; let i = index">
      <td>{{i + 1}}</td>
      <td>{{row.count}}</td>
      <td>
        <a href="{{searchLink(row)}}">{{row.key}}</a>
      </td>
    </tr>
    </tbody>
  </table>
</div>`,
    directives: [
        EveboxSearchLinkComponent
    ]
})
class SimpleReport implements OnInit {

    @Input() private title:string;
    @Input() private rows:any[];
    @Input() private header:string;
    @Input() private searchField:string;

    constructor(private router:Router) {
    }

    ngOnInit() {
    }

    searchLink(row:any) {
        return "#/alerts?q=" + `+${this.searchField}:"${row.key}"`;
    }
}

@Component({
    template: `<div class="alert alert-warning alert-dismissable" role="alert">
  <button type="button" class="close" data-dismiss="alert" aria-label="Close">
    <span aria-hidden="true">&times;</span></button>
  <b>Note:<b></b> These reports are experimental and are subject to change - for
    the better!</b>
</div>

<div class="row">
  <div class="col-md-12">
    <button type="button" class="btn btn-default" (click)="refresh()">Refresh
    </button>
  </div>
</div>

<br/>

<div id="alerts_over_time"></div>

<div class="row">
  <div class="col-md-12">
    <simple-report title="Top Alert Signatures" [rows]="signatureRows"
                   header="Signature"
                   searchField="alert.signature.raw"></simple-report>
  </div>
</div>

<div class="row">
  <div class="col-md-6">
    <simple-report title="Top Alerting Source IPs" [rows]="sourceRows"
                   searchField="src_ip"
                   header="Source"></simple-report>
  </div>
  <div class="col-md-6">
    <simple-report title="Top Alerting Destination IPs" [rows]="destinationRows"
                   searchField="dest_ip"
                   header="Destination"></simple-report>
  </div>
</div>`,
    directives: [
        SimpleReport,
        EveboxSearchLinkComponent,
        ROUTER_DIRECTIVES
    ]
})
export class ReportsComponent implements OnInit, OnDestroy {

    private sourceRows:any[];
    private destinationRows:any[];
    private signatureRows:any[];

    private eventsPerMinute:any[];

    private dispatcherSubscription:any;

    constructor(private appService:AppService,
                private reports:ReportsService) {
    }

    ngOnInit() {

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

        this.sourceRows = undefined;
        this.destinationRows = undefined;
        this.signatureRows = undefined;

        this.reports.alertsReport().then(
            (response:any) => {
                console.log(response);
                this.sourceRows = this.mapAggregation(response, "sources");
                this.destinationRows = this.mapAggregation(response, "destinations");
                this.signatureRows = this.mapAggregation(response, "signatures");

                let data = response.aggregations.alerts_per_minute.buckets.map((x:any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.doc_count
                    }
                });

                MG.data_graphic({
                    title: "Alerts Over Time",
                    data: data,
                    height: 200,
                    target: '#alerts_over_time',
                    x_accessor: 'date',
                    y_accessor: 'value',
                    full_width: true,
                    bar_margin: 0,
                    binned: true,
                    left: 40,
                    right: 10,
                    top: 10,
                });

            });

    }
}