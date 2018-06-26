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

import {BehaviorSubject} from "rxjs";
import {Injectable} from "@angular/core";
import {TopNavService} from "./topnav.service";
import {AppService} from "./app.service";
import {ConfigService} from "./config.service";
import {ToastrService} from "./toastr.service";
import {ApiService} from "./api.service";

import * as moment from "moment";
import {HttpParams} from "@angular/common/http";
import {ClientService} from "./client.service";

declare function require(name: string);

let queue = require("queue");

export interface ResultSet {
    took: number;
    timedOut: boolean;
    count: number;
    events: any[];
    newestTimestamp?: string;
    oldestTimestamp?: string;
}

export interface AlertGroup {
    count: number;
    escalatedCount: number;
    maxTs: string;
    minTs: string;
    event: any;
}

@Injectable()
export class ElasticSearchService {

    private index: string;
    private jobs = queue({concurrency: 4});

    public keywordSuffix = "";

    // Observable for current job count.
    public jobCount$: BehaviorSubject<number> =
            new BehaviorSubject<number>(0);

    public useIpDatatype: boolean = false;

    constructor(private api: ApiService,
                private topNavService: TopNavService,
                private appService: AppService,
                private config: ConfigService,
                private client: ClientService,
                private toastr: ToastrService) {
        this.index = config.getConfig().ElasticSearchIndex;

        try {
            this.keywordSuffix = config.getConfig()["extra"]["elasticSearchKeywordSuffix"];
            this.useIpDatatype = config.getConfig()["extra"]["elasticSearchUseIpDatatype"];
        }
        catch (err) {
            console.log(err);
        }

        console.log("Use Elastic Search keyword suffix: " + this.keywordSuffix);
        console.log("Using Elastic Search IP datatype: " + this.useIpDatatype);
    }

    /**
     * Get the current job size.
     */
    jobSize(): number {
        return this.jobs.length;
    }

    search(query: any): Promise<any> {
        return this.api.post("api/1/query", query)
                .then((response: any) => response,
                        (error: any) => {
                            throw error.json();
                        });
    }

    updateJobCount() {
        this.jobCount$.next(this.jobSize());
    }

    submit(func: any) {

        let p = new Promise<any>((resolve, reject) => {

            this.jobs.push((cb: any) => {
                func().then(() => {
                    cb();
                    resolve();
                    this.updateJobCount();
                }).catch(() => {
                    cb();
                    reject();
                    this.updateJobCount();
                });
            });

            this.updateJobCount();

        });

        this.jobs.start();

        return p;
    }

    asKeyword(keyword: string): string {
        return `${keyword}${this.keywordSuffix}`;
    }

    keywordTerm(keyword: string, value: any): any {
        let field = this.asKeyword(keyword);
        let term = {};
        term[field] = value;
        return {
            term: term
        };
    }

    escalateEvent(event: any): Promise<any> {
        event._source.tags.push("escalated");
        event._source.tags.push("evebox.escalated");
        return this.api.post(`api/1/event/${event._id}/escalate`, {});
    }

    deEscalateEvent(event: any): Promise<any> {
        let idx = event._source.tags.indexOf("escalated");
        if (idx > -1) {
            event._source.tags.splice(idx, 1);
        }
        idx = event._source.tags.indexOf("evebox.escalated");
        if (idx > -1) {
            event._source.tags.splice(idx, 1);
        }
        return this.api.post(`api/1/event/${event._id}/de-escalate`, {});
    }

    /**
     * Archive an event.
     *
     * @param event An Elastic Search document.
     */
    archiveEvent(event: any): Promise<any> {
        return this.submit(() => {
            return this.api.post(`api/1/event/${event._id}/archive`, {});
        });
    }

    escalateAlertGroup(alertGroup: AlertGroup): Promise<string> {
        return this.submit(() => {
            let request = {
                signature_id: alertGroup.event._source.alert.signature_id,
                src_ip: alertGroup.event._source.src_ip,
                dest_ip: alertGroup.event._source.dest_ip,
                min_timestamp: alertGroup.minTs,
                max_timestamp: alertGroup.maxTs,
            };
            console.log(request);
            return this.api.post("api/1/alert-group/star", request);
        });
    }

    archiveAlertGroup(alertGroup: AlertGroup) {
        return this.submit(() => {
            let request = {
                signature_id: alertGroup.event._source.alert.signature_id,
                src_ip: alertGroup.event._source.src_ip,
                dest_ip: alertGroup.event._source.dest_ip,
                min_timestamp: alertGroup.minTs,
                max_timestamp: alertGroup.maxTs,
            };
            return this.api.post("api/1/alert-group/archive", request);
        });
    }

    removeEscalatedStateFromAlertGroup(alertGroup: AlertGroup): Promise<string> {
        return this.submit(() => {
            let request = {
                signature_id: alertGroup.event._source.alert.signature_id,
                src_ip: alertGroup.event._source.src_ip,
                dest_ip: alertGroup.event._source.dest_ip,
                min_timestamp: alertGroup.minTs,
                max_timestamp: alertGroup.maxTs,
            };
            return this.api.post("api/1/alert-group/unstar", request);
        });

    }

    getEventById(id: string): Promise<any> {
        return this.api.get(`api/1/event/${id}`)
                .then((response: any) => {
                    let event = response;

                    // Make sure tags exists.
                    if (!event._source.tags) {
                        event._source.tags = [];
                    }

                    return event;
                });
    }

    findFlow(params: any): Promise<any> {
        return this.api.post("api/1/find-flow", params);
    }

    getAlerts(options: any = {}): Promise<any> {
        let params = new HttpParams();
        let tags: string[] = [];

        if (options.mustHaveTags) {
            options.mustHaveTags.forEach((tag: string) => {
                tags.push(tag);
            });
        }

        if (options.mustNotHaveTags) {
            options.mustNotHaveTags.forEach((tag: string) => {
                tags.push(`-${tag}`);
            });
        }

        params = params.append("tags", tags.join(","));
        params = params.append("time_range", options.timeRange);
        params = params.append("query_string", options.queryString);

        return this.client.get("api/1/alerts", params)
                .toPromise()
                .then((response: any) => {
                    return response.alerts.map((alert: AlertGroup) => {
                        return {
                            event: alert,
                            selected: false,
                            date: moment(alert.maxTs).toDate()
                        };
                    });
                });
    }

    /**
     * Add a time range filter to a query.
     *
     * @param query The query.
     * @param now The time to use as now (a moment object).
     * @param range The time range of the report in seconds.
     */
    addTimeRangeFilter(query: any, now: any, range: number) {
        if (!range) {
            return;
        }

        let then = now.clone().subtract(moment.duration(range, "seconds"));

        query.query.bool.filter.push({
            range: {
                "@timestamp": {
                    gte: `${then.format()}`,
                }
            }
        });
    }

    addSensorNameFilter(query: any, sensor: string) {
        let term = {};
        term[`host${this.keywordSuffix}`] = sensor;
        query.query.bool.filter.push({
            "term": term,
        });
    }

    resolveHostnameForIp(ip: string) {
        let query = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: "event_type"}},
                        {term: {"event_type": "dns"}},
                        this.keywordTerm("dns.rdata", ip),
                    ]
                }
            },
            size: 1,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
        };

        return this.search(query).then((response: any) => {
            if (response.hits.hits.length > 0) {
                let hostname = response.hits.hits[0]._source.dns.rrname;
                return hostname;
            }
        }, error => {
            console.log("Failed to resolve hostname for IP: " + error);
        });
    }

}
