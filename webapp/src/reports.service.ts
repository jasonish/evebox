import {Injectable} from "@angular/core";
import {ElasticSearchService} from "./elasticsearch.service";
import {TopNavService} from "./topnav.service";

@Injectable()
export class ReportsService {

    constructor(private elasticsearch:ElasticSearchService,
                private topNavService:TopNavService) {
    }

    findAlertsGroupedBySourceIp(options:any = {}):any {

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

}