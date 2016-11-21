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

@Injectable()
export class ReportsService {

    private warningShown:boolean = false;

    constructor(private elasticsearch:ElasticSearchService,
                private topNavService:TopNavService,
                private toastr:ToastrService) {
    }

    asKeyword(keyword:string):string {
        return this.elasticsearch.asKeyword(keyword);
    }

    dnsResponseReport(options:any = {}):any {

        let now = moment();
        let range = this.topNavService.getTimeRangeAsSeconds();
        let size:number = options.size || 20;

        let query:any = {
            query: {
                bool: {
                    filter: [
                        {exists: {field: "event_type"}},
                        {term: {event_type: "dns"}},
                        {term: {"dns.type": "answer"}}
                    ]
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                rcodes: {
                    terms: {
                        field: this.asKeyword("dns.rcode"),
                        size: size,
                    }
                },
                top_rdata: {
                    terms: {
                        field: this.asKeyword("dns.rdata"),
                        size: size
                    }
                },
                top_rcode: {
                    terms: {
                        field: this.asKeyword("dns.rcode"),
                        size: size
                    }
                },
            }
        };

        if (options.queryString) {
            query.query.bool.filter.push({
                query_string: {
                    query: options.queryString
                }
            });
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
                bool: {
                    filter: [
                        {exists: {field: "event_type"}},
                        {term: {event_type: "dns"}},
                        {term: {"dns.type": "query"}}
                    ]
                }
            },
            size: 0,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            aggs: {
                top_rrnames: {
                    terms: {
                        field: this.asKeyword("dns.rrname"),
                        size: size
                    }
                },
                top_servers: {
                    terms: {
                        field: this.asKeyword("dest_ip"),
                        size: size
                    }
                },
                top_clients: {
                    terms: {
                        field: this.asKeyword("src_ip"),
                        size: size
                    }
                },
                top_rrtype: {
                    terms: {
                        field: this.asKeyword("dns.rrtype"),
                        size: size
                    }
                }
            }
        };

        if (options.queryString) {
            query.query.bool.filter.push({
                query_string: {
                    query: options.queryString
                }
            });
        }

        this.elasticsearch.addTimeRangeFilter(query, now, range);

        return this.elasticsearch.search(query);
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
