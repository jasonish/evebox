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

    findStats(options:any = {}):any {

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {event_type: "stats"}}
                        ]
                    }
                }
            },
            size: 1000,
            aggs: {
                per_minute: {
                    date_histogram: {
                        field: "@timestamp",
                        interval: "minute"
                    },
                    aggs: {
                        packets: {
                            max: {
                                field: "stats.capture.kernel_drops"
                            }
                        }
                    }
                }
            }
        };

        return this.elasticsearch.search(query);

    }

    getLastStat():any {

        let query:any = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {event_type: "stats"}}
                        ]
                    }
                }
            },
            size: 1,
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
        };

        return this.elasticsearch.search(query);
    }

    guessBestHistogramInterval():any {

        let interval = "minute";

        if (this.topNavService.timeRange) {

            let timeunit = this.topNavService.timeRange.match(/(\d+)(\w+)/)[2];

            switch (timeunit) {
                case "m":
                    interval = "second";
                    break;
                case "h":
                    interval = "minute";
                    break;
                default:
                    interval = "hour";
                    break;
            }

        }

        return interval;
    }

    dnsResponseReport(options:any = {}):any {

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

        // Set time range.
        if (this.topNavService.timeRange) {
            query.query.filtered.filter.and.push({
                range: {
                    timestamp: {gte: `now-${this.topNavService.timeRange}`}
                }
            });
        }

        return this.elasticsearch.search(query);
    }

    dnsRequestReport(options:any = {}):any {

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
                events_over_time: {
                    date_histogram: {
                        field: "@timestamp",
                        interval: this.guessBestHistogramInterval()
                    }
                },
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

        // Set time range.
        if (this.topNavService.timeRange) {
            query.query.filtered.filter.and.push({
                range: {
                    timestamp: {gte: `now-${this.topNavService.timeRange}`}
                }
            });
        }

        return this.elasticsearch.search(query);
    }

    alertsReport(options:any = {}):any {

        let size:number = options.size || 20;

        // Determine what time interval to run the alerts over time histogram
        // with.
        let alerts_over_time_interval = "minute";
        if (this.topNavService.timeRange) {
            let timerange = this.topNavService.timeRange.match(/(\d+)(\w+)/)[1];
            let timeunit = this.topNavService.timeRange.match(/(\d+)(\w+)/)[2];

            if (timeunit == "h" && parseInt(timerange) >= 6) {
                alerts_over_time_interval = "hour";
            }
            else if (timeunit != "h") {
                alerts_over_time_interval = "hour";
            }
        }
        else {
            alerts_over_time_interval = "hour";
        }

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
                alerts_per_minute: {
                    date_histogram: {
                        field: "@timestamp",
                        interval: alerts_over_time_interval
                    }
                },
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
                signatures: {
                    terms: {
                        field: "alert.signature.raw",
                        size: size
                    }
                }
            }
        };

        // Set time range.
        if (this.topNavService.timeRange) {
            query.query.filtered.filter.and.push({
                range: {
                    timestamp: {gte: `now-${this.topNavService.timeRange}`}
                }
            });
        }

        return this.elasticsearch.search(query);
    }

    submitQuery(query:any) {

        // Set time range.
        if (this.topNavService.timeRange) {
            query.query.filtered.filter.and.push({
                range: {
                    timestamp: {gte: `now-${this.topNavService.timeRange}`}
                }
            });
        }

        return this.elasticsearch.search(query);

    }

}