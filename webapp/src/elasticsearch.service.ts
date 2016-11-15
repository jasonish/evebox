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
import {Injectable} from "@angular/core";
import {Http} from "@angular/http";
import {TopNavService} from "./topnav.service";
import {AppService} from "./app.service";
import {ConfigService} from "./config.service";
import {ToastrService} from "./toastr.service";
import {ApiService} from "./api.service";

import moment = require("moment");
var queue = require("queue");

export interface ResultSet {
    took:number;
    timedOut:boolean;
    count:number;
    events:any[];
    newestTimestamp?:string;
    oldestTimestamp?:string;
}

export interface AlertGroup {
    count:number,
    escalatedCount:number,
    newestTs:string,
    oldestTs:string,
    event:any
}

@Injectable()
export class ElasticSearchService {

    private defaultBatchSize:number = 1000;
    private url:string = window.location.pathname + "elasticsearch";
    private index:string;
    private jobs = queue({concurrency: 4});

    // Default to Logstash/ES 2 template unless we can detect Logstash/ES 5
    // usage.
    public keyword:string = "raw";

    constructor(private api:ApiService,
                private topNavService:TopNavService,
                private appService:AppService,
                private config:ConfigService,
                private toastr:ToastrService) {
        this.index = config.getConfig().ElasticSearchIndex;

        try {
            this.keyword = config.getConfig()["extra"]["elasticSearchKeyword"];
        }
        catch (err) {
        }
        console.log("Use Elastic Search keyword " + this.keyword);
    }

    /**
     * Get the current job size.
     */
    jobSize():number {
        return this.jobs.length;
    }

    search(query:any):Promise<any> {
            return this.api.post("api/1/query", query)
                .then((response:any) => response,
                    (error:any) => {
                        throw error.json()
                    });
    }


    bulk(commands:any[]):Promise<any> {
        let request = commands.map(command => {
                return JSON.stringify(command);
            }).join("\n") + "\n";
        return this.api.postRaw("api/1/_bulk?refresh=true", request)
    }

    submit(func:any) {

        let p = new Promise<any>((resolve, reject) => {

            this.jobs.push((cb:any) => {
                func().then(() => {
                    cb();
                    resolve();
                }).catch(() => {
                    cb();
                    reject();
                })
            });

        });

        this.jobs.start();

        return p;
    }

    asKeyword(keyword:string):string {
        return `${keyword}.${this.keyword}`;
    }

    keywordTerm(keyword:string, value:any):any {
        let field = this.asKeyword(keyword);
        let term = {};
        term[field] = value;
        return {
            term: term
        }
    }

    getAlertsInAlertGroup(alertGroup:AlertGroup, options ?:any) {

        // Make sure options is at least an empty object.
        options = options || {};

        let query:any = {
            query: {
                bool: {
                    filter: [
                        {exists: {"field": "event_type"}},
                        {term: {"event_type": "alert"}},
                        {
                            range: {
                                "timestamp": {
                                    gte: alertGroup.oldestTs,
                                    lte: alertGroup.newestTs
                                }
                            }
                        },
                        {term: {"alert.signature_id": alertGroup.event._source.alert.signature_id}},
                        this.keywordTerm("src_ip", alertGroup.event._source.src_ip),
                        this.keywordTerm("dest_ip", alertGroup.event._source.dest_ip),
                    ],
                    must_not: []
                }
            },
            _source: options._source || true,
            size: this.defaultBatchSize
        };

        if (options.filter) {
            options.filter.forEach((q:any) => {
                query.query.bool.filter.push(q);
            })
        }
        if (options.filters) {
            options.filters.forEach((filter:any) => {
                query.query.bool.filter.and.push(filter);
            })
        }

        if (options.must_not) {
            options.must_not.forEach((q:any) => {
                query.query.bool.must_not.push(q);
            });
        }

        return this.search(query);
    }

    addTagsToEventSet(events:any[], tags:string[]) {

        let bulkUpdates = <any[]>[];

        events.forEach((event:any) => {

            let eventTags:any[] = event._source.tags || [];

            tags.forEach((tag:any) => {
                if (eventTags.indexOf(tag) < 0) {
                    eventTags.push(tag);
                }
            });

            bulkUpdates.push({
                update: {
                    "_index": event._index,
                    "_type": event._type,
                    "_id": event._id
                }
            });
            bulkUpdates.push({
                "doc": {
                    tags: eventTags
                }
            });
        });

        return this.bulk(bulkUpdates);
    }

    removeTagsFromEventSet(events:any[], tags:string[]) {

        let bulkUpdates = <any[]>[];

        events.forEach((event:any) => {

            let eventTags:any[] = event._source.tags || [];

            tags.forEach((tag:any) => {
                let idx = eventTags.indexOf(tag);

                if (idx > -1) {
                    eventTags.splice(idx, 1);
                }
            });

            bulkUpdates.push({
                update: {
                    "_index": event._index,
                    "_type": event._type,
                    "_id": event._id
                }
            });
            bulkUpdates.push({
                "doc": {
                    tags: eventTags
                }
            });
        });

        return this.bulk(bulkUpdates);
    }

    removeEscalatedStateFromAlertGroup(alertGroup:AlertGroup):Promise < string > {

        return this.submit(() => {
            return this._removeEscalatedStateFromAlertGroup(alertGroup);
        });

    }

    _removeEscalatedStateFromAlertGroup(alertGroup:AlertGroup):Promise < string > {

        return new Promise<string>((resolve, reject) => {

            return this.getAlertsInAlertGroup(alertGroup, {
                _source: "tags",
                filter: [{term: {tags: "escalated"}}],
            }).then((response:any) => {
                if (response.hits.hits.length == 0) {
                    console.log("No more alerts to de-escalate.");
                    resolve("OK");
                }
                else {
                    console.log(`De-escalated ${response.hits.hits.length} alerts.`);
                    return this.removeTagsFromEventSet(response.hits.hits,
                        ["escalated", "evebox.escalated"])
                        .then(() => {
                            this._removeEscalatedStateFromAlertGroup(alertGroup)
                                .then((response:any) => {
                                    resolve("OK");
                                });
                        });
                }
            });

        });

    }

    escalateEvent(event:any):Promise<any> {

        return this.addTagsToEventSet([event], ["evebox.escalated", "escalated"]);

    }

    /**
     * Archive an event.
     *
     * @param event An Elastic Search document.
     */
    archiveEvent(event:any):Promise<any> {

        return this.submit(() => {
            return this._archiveEvent(event);
        });

    }

    _archiveEvent(event:any):Promise<any> {

        return this.addTagsToEventSet([event], ["evebox.archived", "archived"]);

    }

    escalateAlertGroup(alertGroup:AlertGroup):Promise < string > {
        return this.submit(() => {
            let request = {
                signature_id: alertGroup.event._source.alert.signature_id,
                src_ip: alertGroup.event._source.src_ip,
                dest_ip: alertGroup.event._source.dest_ip,
                min_timestamp: alertGroup.oldestTs,
                max_timestamp: alertGroup.newestTs,
            };
            return this.api.post("api/1/escalate", request);
        });
    }

    archiveAlertGroup(alertGroup:AlertGroup) {
        return this.submit(() => {
            let request = {
                signature_id: alertGroup.event._source.alert.signature_id,
                src_ip: alertGroup.event._source.src_ip,
                dest_ip: alertGroup.event._source.dest_ip,
                min_timestamp: alertGroup.oldestTs,
                max_timestamp: alertGroup.newestTs,
            };
            return this.api.post("api/1/archive", request);
        });
    }

    getEventById(id:string):Promise<any> {
        return this.api.get(`api/1/event/${id}`)
            .then((response:any) => {
                let event = response;

                // Make sure tags exists.
                if (!event._source.tags) {
                    event._source.tags = [];
                }

                return event;
            })
    }

    /**
     * Find events - all events, not just alerts.
     */
    findEvents(options:any = {}):Promise < ResultSet > {

        let query:any = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: "event_type"}},
                    ],
                    must_not: [
                        {term: {event_type: "stats"}}
                    ]
                }
            },
            size: 500,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            timeout: "6s",
        };

        if (options.queryString) {

            query.query.bool.filter.push({
                query_string: {
                    query: options.queryString,
                }
            });
        }

        if (options.timeEnd) {
            query.query.bool.filter.push({
                range: {
                    timestamp: {lte: options.timeEnd}
                }
            });
        }

        if (options.timeStart) {
            query.query.bool.filter.push({
                range: {
                    timestamp: {gte: options.timeStart}
                }
            });
        }

        if (options.filters) {
            options.filters.forEach((filter:any) => {
                query.query.bool.filter.push(filter);
            });
        }

        return this.search(query).then((response:any) => {

            let events = response.hits.hits;

            events.sort((a:any, b:any) => {
                let x = moment(a._source.timestamp);
                let y = moment(b._source.timestamp);
                return y.diff(x);
            });

            let newestTimestamp:any;
            let oldestTimestamp:any;

            if (events.length > 0) {
                newestTimestamp = events[0]._source["@timestamp"];
                oldestTimestamp = events[events.length - 1]._source["@timestamp"];
            }

            let resultSet:ResultSet = {
                took: response.took,
                count: events.length,
                timedOut: response.timed_out,
                events: events,
                newestTimestamp: newestTimestamp,
                oldestTimestamp: oldestTimestamp
            };

            return resultSet;
        });
    }

    getAlerts(options ?:any):Promise < AlertGroup[] > {

        options = options || {};

        let query:any = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: "event_type"}},
                        {term: {event_type: "alert"}}
                    ],
                    must_not: []
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                signatures: {
                    terms: {
                        field: "alert.signature_id",
                        size: 100000,
                    },
                    aggs: {
                        sources: {
                            terms: {
                                field: this.asKeyword("src_ip"),
                                size: 100000
                            },
                            aggs: {
                                destinations: {
                                    terms: {
                                        field: this.asKeyword("dest_ip"),
                                        size: 100000
                                    },
                                    aggs: {
                                        newest: {
                                            top_hits: {
                                                sort: [{"@timestamp": {order: "desc"}}],
                                                size: 1
                                            }
                                        },
                                        oldest: {
                                            top_hits: {
                                                sort: [
                                                    {"@timestamp": {order: "asc"}}
                                                ],
                                                size: 1
                                            }
                                        },
                                        escalated: {
                                            filter: {term: {tags: "escalated"}}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            timeout: "30s"
        };

        if (options.queryString) {
            query.query.bool.filter.push({
                query_string: {
                    query: options.queryString
                }
            })
        }

        if (options.mustNots) {
            options.mustNots.forEach((q:any) => {
                query.query.bool.must_not.push(q);
            });
        }

        if (options.filters) {
            options.filters.forEach((filter:any) => {
                query.query.bool.filter.push(filter);
            });
        }

        if (options.range) {
            if (!options.now) {
                options.now = moment()
            }
            this.addTimeRangeFilter(query, options.now, options.range);
        }

        function unwrapResponse(response:any):AlertGroup[] {

            let events:AlertGroup[] = [];

            if (!response.aggregations) {
                return events;
            }

            // Unwrap from the buckets.
            response.aggregations.signatures.buckets.forEach((sig:any) => {
                sig.sources.buckets.forEach((source:any) => {
                    source.destinations.buckets.forEach((dest:any) => {

                        let event = {

                            // Total number of events in group.
                            count: <number>dest.doc_count,

                            // Number of escalated events.
                            escalatedCount: <number>dest.escalated.doc_count,

                            // The newest (most recent timestamp).
                            newestTs: <string>dest.newest.hits.hits[0]._source.timestamp,

                            // The oldest timestampa.
                            oldestTs: <string>dest.oldest.hits.hits[0]._source.timestamp,

                            // The newest occurrence of the event.
                            event: <any>dest.newest.hits.hits[0]

                        };

                        // Make sure tags exists.
                        if (!event.event._source.tags) {
                            event.event._source.tags = [];
                        }

                        events.push(event);

                    })
                })
            });

            // Sort.
            events.sort((a, b) => {
                let x = moment(a.newestTs);
                let y = moment(b.newestTs);
                return y.diff(x);
            });

            return events;

        }

        return this.search(query).then((response:any) => {

            if (response._shards.total == 0) {
                console.log(`No shards found for index ${this.index}.`);
                this.toastr.error(`No shards found for index ${this.index}`, {
                    title: "Error"
                });
            }

            return unwrapResponse(response)
        });
    }

    /**
     * Add a time range filter to a query.
     *
     * @param query The query.
     * @param now The time to use as now (a moment object).
     * @param range The time range of the report in seconds.
     */
    addTimeRangeFilter(query:any, now:any, range:number) {
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
        })
    }

    addSensorNameFilter(query:any, sensor:string) {
        let term = {};
        term[`host.${this.keyword}`] = sensor;
        query.query.bool.filter.push({
            "term": term,
        });
    }

}
