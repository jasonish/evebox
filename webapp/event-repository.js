/* Copyright (c) 2014-2015 Jason Ish
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

/**
 * A single event is structured like:
 *     {
 *         count: <number of events>
 *         newest: <the most recent occurrence of event>
 *         oldest: <the oldest occurrence of event>
 *     }
 * where newest and oldest are event objects from the lower and upper bounds
 * of the date range. The oldest is only kept to set a reasonable time range
 * on events to archive.
 *
 * When requesting a single event by ID the same structure is used, however
 * count will be 1, and newest and oldest will be the same. This allows the
 * same archive code to be used for a single event by ID and
 * a group of similar events.
 */

import angular from "angular";
import moment from "moment";
import lodash from "lodash";
import queue from "queue";

import * as evebox from "./evebox";

angular.module("app").factory("EventRepository", EventRepository);

EventRepository.$inject = ["$http", "$q", "$timeout"];

class ResultSet {
}

class ElasticSearchResultSet extends ResultSet {

    constructor(response) {
        super();
        this.results = response.hits.hits;
        this.timedOut = response.timed_out;
        this.took = response.took;
        this.total = response.hits.total;

        // Earliest timestamp in the set (oldest).
        this.earliest = _.last(this.results)._source["@timestamp"];

        // Latest timestamp (the most recent).
        this.latest = _.first(this.results)._source["@timestamp"];
    }

}

function EventRepository($http, $q, $timeout, Config) {

    let esUrl = "/elasticsearch";
    let defaultIndex = "logstash-*";

    let q = queue({concurrency: 8});

    let service = {
        getEventsGroupedBySignatureSourceDest: getEventsGroupedBySignatureSourceDest,
        getEventById: getEventById,
        archiveEvent: archiveEvent,
        searchEvents: searchEvents,
        submitArchiveEvent: submitArchiveEvent,
        queueLength: queueLength,
        toggleEscalated: toggleEscalated,
        addEscalated: addEscalated,
        removeEscalated: removeEscalated,
        submit: submit
    };

    return service;

    function queueLength() {
        return q.length;
    }

    function submitArchiveEvent(event) {
        let deferred = $q.defer();
        q.push((cb) => {
            archiveEvent(event).then(response => {
                deferred.resolve(response);
                cb();
            })
        });
        q.start();
        return deferred.promise;
    }

    function submit(func) {
        let deferred = $q.defer();
        q.push((cb) => {
            func().then(cb, cb);
        });
        q.start();
        return deferred.promise;
    }

    function search(query) {
        return $http.post(`${esUrl}/${defaultIndex}/_search`, query)
            .then(response => {
                return response.data;
            }, (error) => {
                console.log("Error:");
                console.log(error);
            });
    }

    function bulk(requests) {
        let request = requests.map(request => {
                return JSON.stringify(request);
            }).join("\n") + "\n";
        return $http.post(`${esUrl}/_bulk?refresh=true`, request);
    }

    function getEventById(eventId) {
        let query = {
            query: {
                filtered: {
                    filter: {
                        term: {
                            _id: eventId
                        }
                    }
                }
            }
        };
        return search(query).then(response => {
            if (response.hits.hits && response.hits.hits.length > 0) {
                return {
                    count: 1,
                    newest: response.hits.hits[0],
                    oldest: response.hits.hits[0]
                };
            }
            else {
                return undefined;
            }
        });
    }

    function getAlertFilters(event) {
        let filters = {
            and: [
                {exists: {field: "event_type"}},
                {term: {event_type: "alert"}},
                {term: {"alert.signature.raw": event.newest._source.alert.signature}},
                {term: {"src_ip.raw": event.newest._source.src_ip}},
                {term: {"dest_ip.raw": event.newest._source.dest_ip}},
                {
                    range: {
                        timestamp: {
                            lte: event.newest._source.timestamp
                        }
                    }
                },
                {
                    range: {
                        timestamp: {
                            gte: event.oldest._source.timestamp
                        }
                    }
                }
            ]
        };

        return filters;
    }

    function addTag(filter, tag) {
        let request = {
            query: {
                filtered: {
                    filter: filter
                }
            },
            size: 1000,
            fields: ["_index", "_type", "_id", "tags"]
        };

        // And a not filter on the tag.
        request.query.filtered.filter.and.push({
            not: {
                term: {
                    tags: tag
                }
            }
        });

        return $q((resolve, reject) => {
            (function execute() {

                search(request).then(response => {

                    if (!response.hits.hits.length) {
                        resolve();
                        return;
                    }

                    let bulkUpdate = buildBulkAddTag(response.hits.hits,
                        tag);

                    if (bulkUpdate.length > 0) {
                        bulk(bulkUpdate).then(execute);
                    }
                    else {
                        execute();
                    }

                });

            })();
        });

    }

    function removeTag(filter, tag) {
        let request = {
            query: {
                filtered: {
                    filter: filter
                }
            },
            size: 1000,
            fields: ["_index", "_type", "_id", "tags"]
        };

        // Limit results to events with the tag.
        request.query.filtered.filter.and.push({
            term: {
                tags: tag
            }
        });

        return $q((resolve, reject) => {
            (function execute() {

                search(request).then(response => {

                    if (!response.hits.hits.length) {
                        resolve();
                        return;
                    }

                    let bulkUpdate = buildBulkRemoveTag(response.hits.hits,
                        tag);

                    if (bulkUpdate.length > 0) {
                        bulk(bulkUpdate).then(execute);
                    }
                    else {
                        execute();
                    }

                });

            })();
        });

    }

    function addEscalated(event) {

        let filters = getAlertFilters(event);

        // If the event is not archived add a filter to limit the operation to
        // inbox in the inbox (not archived).
        if (!evebox.hasTag(event, "archived")) {
            filters.and.push({
                not: {
                    term: {
                        tags: "archived"
                    }
                }
            })
        }

        return addTag(filters, "escalated");
    }

    function removeEscalated(event) {

        let filters = getAlertFilters(event);

        // If the event is not archived add a filter to limit the operation to
        // inbox in the inbox (not archived).
        if (!evebox.hasTag(event, "archived")) {
            filters.and.push({
                not: {
                    term: {
                        tags: "archived"
                    }
                }
            })
        }

        return removeTag(filters, "escalated");
    }

    function toggleEscalated(event) {

        let remove = event.escalated == event.count;

        let filters = getAlertFilters(event);

        // If the event is not archived add a filter to limit the operation to
        // the inbox (not archived).
        if (!evebox.hasTag(event, "archived")) {
            filters.and.push({
                not: {
                    term: {
                        tags: "archived"
                    }
                }
            })
        }

        if (remove) {
            return removeTag(filters, "escalated");
        }
        else {
            return addTag(filters, "escalated");
        }

    }

    function archiveEvent(event) {
        let filters = getAlertFilters(event);

        filters.and.push({
            not: {
                term: {
                    tags: "archived"
                }
            }
        });

        return addTag(filters, "archived");
    }

    /**
     * Take a list of events (ES docs) and create a bulk update to add the
     * specified tag to all of them.
     */
    function buildBulkAddTag(events, tag) {
        let bulkUpdates = [];

        events.forEach(event => {

            let tags = [];

            if (event.fields) {
                if (event.fields.tags) {
                    tags = event.fields.tags;
                    if (tags.indexOf(tag) > -1) {
                        return;
                    }
                }
            }

            tags.push(tag);

            bulkUpdates.push({
                "update": {
                    "_index": event._index,
                    "_type": event._type,
                    "_id": event._id
                }
            });
            bulkUpdates.push({
                "doc": {
                    tags: tags
                }
            });

        });

        return bulkUpdates;
    }

    function buildBulkRemoveTag(events, tag) {
        let bulkUpdates = [];

        events.forEach(event => {

            let tags = [];

            if (event.fields) {
                if (event.fields.tags) {
                    tags = event.fields.tags;
                }
            }

            let idx = tags.indexOf("escalated");

            if (idx < 0) {
                return;
            }

            tags.splice(idx, 1);

            bulkUpdates.push({
                "update": {
                    "_index": event._index,
                    "_type": event._type,
                    "_id": event._id
                }
            });
            bulkUpdates.push({
                "doc": {
                    tags: tags
                }
            });

        });

        return bulkUpdates;
    }

    function searchEvents(options) {

        console.log("EventRepository.searchEvents");
        console.log("- options: " + JSON.stringify(options));

        let reverse = false;
        let queryString = "*";
        let sort = [];

        if (options && options.queryString) {
            queryString = options.queryString;
        }

        let filters = [
            {exists: {field: "event_type"}},
            {not: {term: {event_type: "stats"}}}
        ];

        if (options.timeStart) {
            console.log("Setting timeStart to " + options.timeEnd);
            reverse = true;
            filters.push({
                range: {
                    timestamp: {
                        gte: options.timeStart
                    }
                }
            });
        }
        else if (options.timeEnd) {
            console.log("Setting timeEnd to " + options.timeEnd);
            filters.push({
                range: {
                    timestamp: {
                        lte: options.timeEnd
                    }
                }
            });
            sort.push({"@timestamp": {order: "desc"}});
        }
        else if (options.timeRange) {
            filters.push({
                range: {
                    timestamp: {
                        gte: `now-${options.timeRange}`
                    }
                }
            });
            sort.push({"@timestamp": {order: "desc"}});
        }

        let query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: queryString
                        }
                    },
                    filter: {
                        and: filters
                    }
                }
            }
        };

        query.size = options.size || 100;
        query.from = options.from || 0;
        query.sort = sort;
        query.timeout = 1;

        return search(query).then(response => {
            if (reverse) {
                console.log("Reversing hits.");
                response.hits.hits.reverse();
            }
            return new ElasticSearchResultSet(response);
        });
    }

    function getEventsGroupedBySignatureSourceDest(options) {

        console.log("EventRepository.getEventsGroupedBySignatureSourceDest:");
        console.log("- options: " + JSON.stringify(options));

        if (options === undefined) {
            options = {};
        }

        let filters = [];

        filters.push({exists: {field: "event_type"}});
        filters.push({term: {event_type: "alert"}});

        if (options.filters) {
            options.filters.forEach(filter => {
                filters.push(filter);
            });
        }

        if (options.timeRange) {
            filters.push({
                range: {
                    timestamp: {
                        gte: `now-${options.timeRange}`
                    }
                }
            })
        }

        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: options.queryString || "*"
                        }
                    },
                    filter: {and: filters}
                }
            },
            size: 0,

            // Probably not needed when size is 0.
            sort: [
                {"@timestamp": {order: "desc"}}
            ]
        };

        let signatures = {
            terms: {
                field: "alert.signature.raw",
                size: 0
            }
        };

        let sources = {
            terms: {
                field: "src_ip.raw",
                size: 0
            }
        };


        let destinations = {
            terms: {
                field: "dest_ip.raw",
                size: 0
            }
        };


        let newest = {
            top_hits: {
                sort: [
                    {"@timestamp": {order: "desc"}}
                ],
                size: 1
            }
        };

        let oldest = {
            top_hits: {
                sort: [
                    {"@timestamp": {order: "asc"}}
                ],
                size: 1
            }
        };

        let escalated = {
            filter: {
                term: {
                    tags: "escalated"
                }
            }
        };

        // By signature->source->destination.
        query.aggs = {signatures: signatures};
        query.aggs.signatures.aggs = {sources: sources};
        query.aggs.signatures.aggs.sources.aggs = {destinations: destinations};
        query.aggs.signatures.aggs.sources.aggs.destinations.aggs = {
            newest: newest,
            oldest: oldest,
            escalated: escalated
        };

        query.timeout = 1000;

        return search(query).then(response => {

            //console.log(
            //    "EventRepository.getEventsGroupedBySignatureSourceDest: Response:");
            //console.log(response);

            let events = [];

            if (!response.aggregations) {
                return events;
            }

            response.aggregations.signatures.buckets.forEach(sig => {
                sig.sources.buckets.forEach(source => {
                    source.destinations.buckets.forEach(destination => {

                        let event = {
                            count: destination.doc_count,
                            newest: destination.newest.hits.hits[0],
                            oldest: destination.oldest.hits.hits[0],
                            escalated: destination.escalated.doc_count
                        };

                        events.push(event);

                        // Ensure there is a tags array, even if empty.
                        if (!event.newest._source.tags) {
                            event.newest._source.tags = [];
                        }
                    })
                })
            });

            // Sort events, newest first.
            events.sort((a, b) => {
                let x = moment(a.newest._source.timestamp).unix();
                let y = moment(b.newest._source.timestamp).unix();
                return y - x;
            });

            console.log(
                "EventRepository.getEventsGroupedBySignatureSourceDest: Events: " +
                events.length);

            return events;

        });
    }

}
