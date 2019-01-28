/* Copyright (c) 2016-2019 Jason Ish
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
import {ReportsService} from "../reports.service";
import {AppEventCode, AppService} from "../../app.service";
import {EveboxFormatIpAddressPipe} from "../../pipes/format-ipaddress.pipe";
import {EveboxSubscriptionTracker} from "../../subscription-tracker";
import {ActivatedRoute, Params} from "@angular/router";
import {ApiService, ReportAggOptions} from "../../api.service";
import {TopNavService} from "../../topnav.service";

import * as moment from "moment";

@Component({
    templateUrl: "./dns-report.component.html",
})
export class DNSReportComponent implements OnInit, OnDestroy {

    eventsOverTime: any[];

    topRrnames: any[];
    topRdata: any[];
    topRrtypes: any[];
    topRcodes: any[];
    topServers: any[];
    topClients: any[];

    loading = 0;

    queryString = "";

    subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

    constructor(private route: ActivatedRoute,
                private reports: ReportsService,
                private appService: AppService,
                private api: ApiService,
                private topNavService: TopNavService,
                private reportsService: ReportsService,
                private formatIpAddressPipe: EveboxFormatIpAddressPipe) {
    }

    ngOnInit() {

        this.subTracker.subscribe(this.route.params, (params: Params) => {
            this.queryString = params["q"] || "";
            this.refresh();
        });

        this.subTracker.subscribe(this.appService, (event: any) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    ngOnDestroy() {
        this.subTracker.unsubscribe();
    }

    mapAddressAggregation(items: any[]) {
        return items.map((item: any) => {

            let key = item.key;

            // If key looks like an IP address, format it.
            if (key.match(/\d*\.\d*\.\d*\.\d*/)) {
                key = this.formatIpAddressPipe.transform(key);
            }

            return {
                key: key,
                count: item.doc_count,
            };

        });
    }

    mapAggregation(items: any[]) {
        return items.map((item: any) => {
            return {
                key: item.key,
                count: item.doc_count,
            };
        });
    }

    load(fn: any) {
        this.loading++;
        fn().then(() => {
        }).catch((err) => {
        }).then(() => {
            this.loading--;
        })
    }

    refresh() {
        let size = 10;
        let range = this.topNavService.getTimeRangeAsSeconds();

        let aggOptions: ReportAggOptions = {
            eventType: "dns",
            dnsType: "answer",
            timeRange: range,
            queryString: this.queryString,
            size: size,
        };

        // Top response codes.
        this.load(() => {
            return this.api.reportAgg("dns.rcode", Object.assign({
                dnsType: "answer",
            }, aggOptions))
                .then((response: any) => {
                    this.topRcodes = response.data;
                });
        });

        // Switch to queries.
        aggOptions.dnsType = "query";

        // Top request rrnames.
        this.load(() => {
            return this.api.reportAgg("dns.rrname", aggOptions)
                .then((response: any) => {
                    this.topRrnames = response.data;
                });
        });

        // Top request rrtypes.
        this.load(() => {
            return this.api.reportAgg("dns.rrtype", aggOptions)
                .then((response: any) => {
                    this.topRrtypes = response.data;
                });
        });

        // Top DNS clients.
        this.load(() => {
            return this.api.reportAgg("src_ip", aggOptions)
                .then((response: any) => {
                    this.topClients = response.data;
                });
        });

        // Top DNS servers.
        this.load(() => {
            return this.api.reportAgg("dest_ip", aggOptions)
                .then((response: any) => {
                    this.topServers = response.data;
                });
        });

        // Queries over time histogram.
        this.load(() => {
            return this.api.reportHistogram({
                timeRange: range,
                interval: this.reportsService.histogramTimeInterval(range),
                eventType: "dns",
                dnsType: "query",
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

    }
}