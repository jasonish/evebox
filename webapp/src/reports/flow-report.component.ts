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
import {EveboxMetricsGraphicComponent} from "../metricgraphics.component";
import {ReportsService} from "./reports.service";
import {EveboxSubscriptionService} from "../subscription.service";
import {AppService, AppEvent, AppEventCode} from "../app.service";
import {EveboxLoadingSpinnerComponent} from "../loading-spinner.component";

import moment = require("moment");
import {EveboxHumanizeService} from "../humanize.service";
import {EveboxReportDataTable} from "./dns-report.component";

@Component({
    template: `<div [ngClass]="{'evebox-opacity-50': loading > 0}">

  <loading-spinner [loading]="loading > 0"></loading-spinner>

  <metrics-graphic *ngIf="eventsOverTime"
                   graphId="eventsOverTime"
                   title="Netflow Events Over Time"
                   [data]="eventsOverTime"></metrics-graphic>

  <metrics-graphic *ngIf="averageAgeOverTime"
                   graphId="averageAgeOverTime"
                   title="Average Flow Age Over Time"
                   [data]="averageAgeOverTime"></metrics-graphic>

  <metrics-graphic *ngIf="maxAgeOverTime"
                   graphId="maxAgeOverTime"
                   title="Max Flow Age Over Time"
                   [data]="maxAgeOverTime"></metrics-graphic>

  </div>`,
    directives: [
        EveboxMetricsGraphicComponent,
        EveboxLoadingSpinnerComponent,
        EveboxReportDataTable,
    ]
})
export class FlowReportComponent implements OnInit, OnDestroy {

    private eventsOverTime:any[];

    private averageAgeOverTime:any[];
    private maxAgeOverTime:any[];

    private loading:number = 0;

    constructor(private appService:AppService,
                private ss:EveboxSubscriptionService,
                private reportsService:ReportsService,
                private humanize:EveboxHumanizeService) {
    }

    ngOnInit() {

        this.refresh();

        this.ss.subscribe(this, this.appService, (event:AppEvent) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.ss.unsubscribe(this);
    }

    refresh() {

        this.loading++;

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            // Somewhat limit to eve events only.
                            {exists: {field: "event_type"}},

                            {term: {event_type: "flow"}}
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                events_over_time: {
                    date_histogram: {
                        field: "@timestamp",
                        interval: this.reportsService.guessBestHistogramInterval(),
                    },
                    aggs: {
                        averageAge: {
                            avg: {
                                field: "flow.age",
                            }
                        },
                        maxAge: {
                            max: {
                                field: "flow.age",
                            }
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
                },
            }
        };

        this.reportsService.submitQuery(query).then((response:any) => {

            this.averageAgeOverTime = [];
            this.maxAgeOverTime = [];

            this.eventsOverTime = response.aggregations.events_over_time.buckets.map((item:any) => {

                let date = moment(item.key).toDate();

                this.averageAgeOverTime.push({
                    date: date,
                    value: item.averageAge.value,
                });

                this.maxAgeOverTime.push({
                    date: date,
                    value: item.maxAge.value,
                });

                return {
                    date: date,
                    value: item.doc_count
                }
            });

            this.loading--;

        });

    }
}