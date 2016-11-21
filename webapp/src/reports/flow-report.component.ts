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

import {Component, OnInit, OnDestroy} from "@angular/core";
import {ActivatedRoute, Params} from "@angular/router";
import {ReportsService} from "./reports.service";
import {AppService, AppEvent, AppEventCode} from "../app.service";
import {TopNavService} from "../topnav.service";
import {ElasticSearchService} from "../elasticsearch.service";
import {loadingAnimation} from "../animations";
import {EveboxSubscriptionTracker} from "../subscription-tracker";
import {ApiService} from "../api.service";

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
                   graphId="eventsOverTime"
                   title="Netflow Events Over Time"
                   [data]="eventsOverTime"></metrics-graphic>

  <div class="row">

    <div class="col-md-6">
      <report-data-table *ngIf="topClientsByFlows"
                         title="Top Clients By Flow Count"
                         [rows]="topClientsByFlows"
                         [headers]="['Flows', 'Client IP']"></report-data-table>
    </div>

    <div class="col-md-6">
      <report-data-table *ngIf="topServersByFlows"
                         title="Top Servers By Flow Count"
                         [rows]="topServersByFlows"
                         [headers]="['Flows', 'Server IP']"></report-data-table>
    </div>

  </div>

  <div *ngIf="topFlowsByAge" class="panel panel-default">
    <div class="panel-heading">
      <b>Top Flows by Age</b>
    </div>
    <eveboxEventTable2 [rows]="topFlowsByAge"
                       [showEventType]="false"
                       [showActiveEvent]="false"></eveboxEventTable2>
  </div>

</div>`,
    animations: [
        loadingAnimation,
    ]
})
export class FlowReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[];

    private topClientsByFlows:any[];
    private topServersByFlows:any[];

    private topFlowsByAge:any[];

    private loading:number = 0;

    private queryString:string = "";

    private subTracker:EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    constructor(private appService:AppService,
                private route:ActivatedRoute,
                private reportsService:ReportsService,
                private topNavService:TopNavService,
                private api:ApiService,
                private elasticsearch:ElasticSearchService) {
    }

    ngOnInit() {

        this.subTracker.subscribe(this.route.params, (params:Params) => {
            this.queryString = params["q"] || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event:AppEvent) => {
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

        this.loading++;

        let range = this.topNavService.getTimeRangeAsSeconds();
        let now = moment();

        this.load(() => {
            return this.api.reportHistogram({
                timeRange: range,
                interval: this.reportsService.histogramTimeInterval(range),
                eventType: "flow",
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

        let query:any = {
            query: {
                bool: {
                    filter: [
                        // Somewhat limit to eve events only.
                        {exists: {field: "event_type"}},
                        {term: {event_type: "flow"}}
                    ]
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                topClientsByFlows: {
                    terms: {
                        field: `src_ip.${this.elasticsearch.keyword}`,
                        order: {
                            "_count": "desc",
                        }
                    }
                },
                topServersByFlows: {
                    terms: {
                        field: `dest_ip.${this.elasticsearch.keyword}`,
                        order: {
                            "_count": "desc",
                        }
                    }
                },
                topFlowsByAge: {
                    top_hits: {
                        sort: [
                            {"flow.age": {order: "desc"}}
                        ],
                        size: 10,
                    }
                }
            }
        };

        if (this.queryString && this.queryString != "") {
            query.query.filtered.query = {
                query_string: {
                    query: this.queryString
                }
            }
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.reportsService.submitQuery(query).then((response:any) => {

            this.topClientsByFlows = response.aggregations.topClientsByFlows.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                };
            });

            this.topServersByFlows = response.aggregations.topServersByFlows.buckets.map((bucket:any) => {
                return {
                    key: bucket.key,
                    count: bucket.doc_count,
                };
            });

            this.topFlowsByAge = response.aggregations.topFlowsByAge.hits.hits;

            this.loading--;

        });

    }
}