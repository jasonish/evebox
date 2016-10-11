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
import {ElasticSearchService} from "../elasticsearch.service";
import {TopNavService} from "../topnav.service";
import {ToastrService} from "../toastr.service";
import moment = require("moment");
import UnitOfTime = moment.UnitOfTime;

@Injectable()
export class ReportsService {

    private warningShown:boolean = false;

    constructor(private elasticsearch:ElasticSearchService,
                private topNavService:TopNavService,
                private toastr:ToastrService) {
    }

    showWarning() {
        if (this.warningShown) {
            return;
        }
        this.warningShown = true;
        this.toastr.warning("Reports are experimental are are subject to change.", {
            title: "Warning",
            closeButton: true,
            timeOut: 3000,
            preventDuplicates: true,
        });
    }

    dnsResponseReport(options:any = {}):any {

        let now = moment();
        let range = this.topNavService.getTimeRangeAsSeconds();
        let size:number = options.size || 20;

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {event_type: "dns"}},
                            {term: {"dns.type": "answer"}}
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "dest"}}
            ],
            aggs: {
                rcodes: {
                    terms: {
                        field: "dns.rcode.raw",
                        size: size,
                    }
                },
                top_rdata: {
                    terms: {
                        field: "dns.rdata.raw",
                        size: size
                    }
                },
                top_rcode: {
                    terms: {
                        field: "dns.rcode.raw",
                        size: size
                    }
                },
            }
        };

        if (options.queryString) {
            query.query.filtered.query = {
                query_string: {
                    query: options.queryString
                }
            }
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        return this.elasticsearch.search(query);
    }

    dnsRequestReport(options:any = {}):any {

        let now:any = moment();
        let range:number = this.topNavService.getTimeRangeAsSeconds();
        let size:number = options.size || 20;

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {event_type: "dns"}},
                            {term: {"dns.type": "query"}}
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "dest"}}
            ],
            aggs: {
                top_rrnames: {
                    terms: {
                        field: "dns.rrname.raw",
                        size: size
                    }
                },
                top_servers: {
                    terms: {
                        field: "dest_ip.raw",
                        size: size
                    }
                },
                top_clients: {
                    terms: {
                        field: "src_ip.raw",
                        size: size
                    }
                },
                top_rrtype: {
                    terms: {
                        field: "dns.rrtype.raw",
                        size: size
                    }
                }
            }
        };

        if (options.queryString) {
            query.query.filtered.query = {
                query_string: {
                    query: options.queryString
                }
            }
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);
        this.addEventsOverTimeAggregation(query, now, range);

        return this.elasticsearch.search(query);
    }

    alertsReport(options:any = {}):any {

        let range:number = this.topNavService.getTimeRangeAsSeconds();
        let now:any = moment();
        let size:number = options.size || 20;

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            // Somewhat limit to eve events only.
                            {exists: {field: "event_type"}},

                            // And only look at alerts.
                            {term: {event_type: "alert"}}
                        ]
                    }
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                sources: {
                    terms: {
                        field: "src_ip.raw",
                        size: size
                    }
                },
                destinations: {
                    terms: {
                        field: "dest_ip.raw",
                        size: size
                    }
                },
                src_ports: {
                    terms: {
                        field: "src_port",
                        size: size,
                    }
                },
                dest_ports: {
                    terms: {
                        field: "dest_port",
                        size: size,
                    }
                },
                signatures: {
                    terms: {
                        field: "alert.signature.raw",
                        size: size
                    }
                },
                categories: {
                    terms: {
                        field: "alert.category.raw",
                        size: size,
                    }
                }
            }
        };

        if (options.queryString) {
            query.query.filtered.query = {
                query_string: {
                    query: options.queryString
                }
            }
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);
        this.addEventsOverTimeAggregation(query, now, range);

        return this.elasticsearch.search(query);
    }

    addEventsOverTimeAggregation(query:any, now:any, range:number) {

        query.aggs.events_over_time = {
            date_histogram: {
                field: "@timestamp",
                interval: this.histogramTimeInterval(range),
                min_doc_count: 0,
            }
        };

        if (range) {
            let then = now.clone().subtract(moment.duration(range, "seconds"));
            query.aggs.events_over_time.date_histogram.extended_bounds = {
                min: then.format(),
                max: now.format(),
            }
        }
    }

    histogramTimeInterval(range:number):string {
        let interval:string = "day";

        if (range == 0) {
            return "day";
        }
        else if (range <= 60) {
            // Minute or less.
            interval = "second";
        }
        else if (range <= 3600 * 6) {
            // 6 hours or or less.
            interval = "minute";
        }
        else if (range <= 86400) {
            // Day or less.
            interval = "hour";
        }

        console.log(`Returning interval: ${interval}.`);

        return interval;
    }

    submitQuery(query:any) {
        return this.elasticsearch.search(query);
    }

}