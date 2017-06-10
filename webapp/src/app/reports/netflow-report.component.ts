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

import {Component, OnInit, OnDestroy} from '@angular/core';
import {Params, ActivatedRoute} from '@angular/router';
import {ReportsService} from './reports.service';
import {AppService, AppEvent, AppEventCode} from '../app.service';
import {ToastrService} from '../toastr.service';
import {TopNavService} from '../topnav.service';
import {ElasticSearchService} from '../elasticsearch.service';
import {EveboxSubscriptionTracker} from '../subscription-tracker';
import {loadingAnimation} from '../animations';
import {humanizeFileSize} from '../humanize.service';
import {ApiService} from '../api.service';

import * as moment from 'moment';

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

  <div *ngIf="noEvents" style="text-align: center;">
    <hr/>
    No netflow events found.
    <hr/>
  </div>

  <metrics-graphic *ngIf="eventsOverTime"
                   graphId="eventsOverTime"
                   title="Netflow Events Over Time"
                   [data]="eventsOverTime"></metrics-graphic>

  <div class="row">

    <div class="col-md-6">

      <report-data-table *ngIf="topSourcesByBytes"
                         title="Top Sources by Bytes"
                         [rows]="topSourcesByBytes"
                         [headers]="['#', 'Source']"></report-data-table>

      <report-data-table *ngIf="topSourcesByPackets"
                         title="Top Sources by Packets"
                         [rows]="topSourcesByPackets"
                         [headers]="['#', 'Source']">
      </report-data-table>

     </div>

    <div class="col-md-6">

      <report-data-table *ngIf="topDestinationsByBytes"
                         title="Top Destinations By Bytes"
                         [rows]="topDestinationsByBytes"
                         [headers]="['#', 'Destination']"></report-data-table>

      <report-data-table *ngIf="topDestinationsByPackets"
                         title="Top Destinations by Packets"
                         [rows]="topDestinationsByPackets"
                         [headers]="['#', 'Destination']">
      </report-data-table>

    </div>

  </div>

  <div *ngIf="topByBytes" class="panel panel-default">
    <div class="panel-heading">
      <b>Top Flows by Bytes</b>
    </div>
    <eveboxEventTable2 [rows]="topByBytes"
                       [showEventType]="false"
                       [showActiveEvent]="false"></eveboxEventTable2>
  </div>

  <div *ngIf="topFlowsByPackets" class="panel panel-default">
    <div class="panel-heading">
      <b>Top Flows by Packets</b>
    </div>
    <eveboxEventTable2 [rows]="topFlowsByPackets"
                       [showEventType]="false"
                       [showActiveEvent]="false"></eveboxEventTable2>
  </div>

</div>`,
    animations: [
        loadingAnimation,
    ]
})
export class NetflowReportComponent implements OnInit, OnDestroy {

    eventsOverTime: any[];

    topSourcesByBytes: any[];
    topSourcesByPackets: any[];

    topDestinationsByBytes: any[];
    topDestinationsByPackets: any[];

    topByBytes: any[];
    topFlowsByPackets: any[];

    loading = 0;

    queryString = '';

    // A flag that will be set to true if not events to report on were found.
    noEvents = false;

    subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    constructor(private reportsService: ReportsService,
                private elasticsearch: ElasticSearchService,
                private appService: AppService,
                private route: ActivatedRoute,
                private toastr: ToastrService,
                private api: ApiService,
                private topNavService: TopNavService) {
    }

    ngOnInit() {

        this.subTracker.subscribe(this.route.params, (params: Params) => {
            this.queryString = params['q'] || '';
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event: AppEvent) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.subTracker.unsubscribe();
    }

    refresh() {

        this.checkForEvents().then((hasEvents: boolean) => {
            if (hasEvents) {
                this.load();
            }
            else {
                this.noEvents = true;
                this.toastr.warning('No netflow events found.');
            }
        });

    }

    checkForEvents() {

        let query: any = {
            query: {
                bool: {
                    filter: [
                        // Somewhat limit to eve events of netflow only.
                        {exists: {field: 'event_type'}},
                        {term: {event_type: 'netflow'}}
                    ]
                }
            },
            size: 0,
        };

        return this.elasticsearch.search(query).then((response: any) => {
            return response.hits.total > 0;
        });
    }

    wrapLoad(fn: any) {
        this.loading++;
        fn().then(() => {
            this.loading--;
        });
    }

    load() {

        this.loading++;

        let range = this.topNavService.getTimeRangeAsSeconds();
        let now = moment();

        this.wrapLoad(() => {
            return this.api.reportHistogram({
                timeRange: range,
                interval: this.reportsService.histogramTimeInterval(range),
                eventType: 'netflow',
                queryString: this.queryString,
            }).then((response: any) => {
                this.eventsOverTime = response.data.map((x: any) => {
                    return {
                        date: moment(x.key).toDate(),
                        value: x.count
                    };
                });
            });
        });

        let params: any = {
            queryString: this.queryString,
        };

        if (range > 0) {
            params.timeRange = `${range}s`;
        }

        this.wrapLoad(() => {
            params.sortBy = 'netflow.pkts';
            return this.api.getWithParams('api/1/netflow', params)
                .then((response: any) => {
                    this.topFlowsByPackets = response.data;
                });
        });

        this.wrapLoad(() => {
            params.sortBy = 'netflow.bytes';
            return this.api.getWithParams('api/1/netflow', params)
                .then((response: any) => {
                    this.topByBytes = response.data;
                });
        });

        let query: any = {
            query: {
                bool: {
                    filter: [
                        // Somewhat limit to eve events of netflow only.
                        {exists: {field: 'event_type'}},
                        {term: {event_type: 'netflow'}}
                    ]
                }
            },
            size: 0,
            sort: [
                {'@timestamp': {order: 'desc'}}
            ],
            aggs: {
                sourcesByBytes: {
                    terms: {
                        field: this.elasticsearch.asKeyword('src_ip'),
                        order: {
                            'bytes': 'desc'
                        },
                    },
                    aggs: {
                        bytes: {
                            sum: {
                                field: 'netflow.bytes'
                            }
                        }
                    }
                },
                sourcesByPackets: {
                    terms: {
                        field: this.elasticsearch.asKeyword('src_ip'),
                        order: {
                            'packets': 'desc'
                        }
                    },
                    aggs: {
                        packets: {
                            sum: {
                                field: 'netflow.pkts'
                            }
                        }
                    }
                },
                topDestinationsByBytes: {
                    terms: {
                        field: this.elasticsearch.asKeyword('dest_ip'),
                        order: {
                            'bytes': 'desc'
                        },
                    },
                    aggs: {
                        bytes: {
                            sum: {
                                field: 'netflow.bytes',
                            }
                        }
                    }
                },
                topDestinationsByPackets: {
                    terms: {
                        field: this.elasticsearch.asKeyword('dest_ip'),
                        order: {
                            'packets': 'desc'
                        },
                    },
                    aggs: {
                        packets: {
                            sum: {
                                field: 'netflow.pkts'
                            }
                        }
                    }
                },
            }
        };

        if (this.queryString && this.queryString != '') {
            query.query.bool.filter.push({
                query_string: {
                    query: this.queryString
                }
            });
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        this.elasticsearch.search(query).then((response: any) => {

            if (response.error) {
                console.log('Errors returned:');
                console.log(response.error);
                let error = response.error;
                if (error.root_cause && error.root_cause.length > 0) {
                    this.toastr.error(error.root_cause[0].reason);
                }
                this.loading--;
                return;
            }

            console.log(response);
            console.log(response.aggregations);

            this.topSourcesByBytes = response.aggregations.sourcesByBytes.buckets.map((bucket: any) => {
                return {
                    key: bucket.key,
                    count: humanizeFileSize(bucket.bytes.value),
                };
            });

            this.topDestinationsByBytes = response.aggregations.topDestinationsByBytes.buckets.map((bucket: any) => {
                return {
                    key: bucket.key,
                    count: humanizeFileSize(bucket.bytes.value),
                };
            });

            this.topSourcesByPackets = response.aggregations.sourcesByPackets.buckets.map((bucket: any) => {
                return {
                    key: bucket.key,
                    count: bucket.packets.value,
                };
            });

            this.topDestinationsByPackets = response.aggregations.topDestinationsByPackets.buckets.map((bucket: any) => {
                return {
                    key: bucket.key,
                    count: bucket.packets.value,
                };
            });

            this.loading--;

        }, error => {
            console.log('Search error:');
            console.log(error);
            this.loading--;
        });
    }

}