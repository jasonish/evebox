/* Copyright (c) 2014 Jason Ish
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

'use strict';

(function() {

    /**
     * EventRepository service.
     *
     * The idea of this service is provide a level of abstraction over
     * ElasticSearch.
     */
    app.factory("EventRepository",
        function($q, ElasticSearch, Config, NotificationService, printf) {

            var service = {};

            /**
             * ES top hits aggregation to extract the most recent alert in the
             * aggregation.
             */
            service.latestEventAgg = {
                "latest_event": {
                    "top_hits": {
                        "sort": [
                            {
                                "@timestamp": {
                                    "order": "desc"
                                }
                            }
                        ],
                        "_source": {
                            "include": [
                                "timestamp",
                                "@timestamp",
                                "alert.severity",
                                "alert.category"
                            ]
                        },
                        "size": 1
                    }
                }
            };

            /**
             * Aggregate by signature query.
             */
            service.aggregateBySignature = {
                "signature": {
                    "terms": {
                        "field": "alert.signature.raw",
                        "size": 0
                    },
                    "aggs": service.latestEventAgg
                }
            };

            /**
             * Aggregate by signature then source address query.
             */
            service.aggregateBySignatureSrc = {
                "signature": {
                    "terms": {
                        "field": "alert.signature.raw",
                        "size": 0
                    },
                    "aggs": {
                        "source_addrs": {
                            "terms": {
                                "field": "src_ip.raw",
                                "size": 0
                            },
                            "aggs": service.latestEventAgg
                        }
                    }
                }
            };

            var latestEventAggregationTemplate = {
                "latest": {
                    "top_hits": {
                        "sort": [
                            {
                                "@timestamp": {
                                    "order": "desc"
                                }
                            }
                        ],
                        "size": 1
                    }
                }
            };

            var buildEventQuery = function(options) {

                var size = options.size || Config.elasticSearch.size;

                var query = {
                    query: {
                        filtered: {
                            query: {
                                query_string: {
                                    query: options.query || "*"
                                }
                            },
                            filter: {
                                and: [
                                    {
                                        "exists": {"field": "event_type"}
                                    }
                                ]

                            }
                        }
                    },
                    size: size,
                    sort: [
                        {"@timestamp": {order: "desc"}}
                    ]
                };

                if (options.page != undefined && options.page > 1) {
                    query.from = size * (options.page - 1);
                }

                if ("filters" in options) {
                    _.forEach(options.filters, function(filter) {
                        query.query.filtered.filter.and.push(filter);
                    });
                }

                return query;
            };

            /**
             * Build an alert query grouped by signature.
             */
            var buildAlertQueryGroupedBySignature = function(options) {
                var query = buildEventQuery(options);
                query.size = 0
                delete(query.sort);
                query.query.filtered.filter.and.push({
                    term: {event_type: "alert"}
                });
                query.aggs = {
                    "signature": {
                        "terms": {
                            "field": "alert.signature.raw",
                            "size": 0
                        },
                        "aggs": latestEventAggregationTemplate
                    }
                };
                return query;
            };

            var buildAlertQueryGroupedBySignatureAndSource = function(options) {
                var query = buildEventQuery(options);
                delete(query.sort);
                query.size = 0;
                query.query.filtered.filter.and.push({
                    term: {event_type: "alert"}
                });
                query.aggs = {
                    "signature": {
                        "terms": {
                            "field": "alert.signature.raw",
                            "size": 0
                        },
                        "aggs": {
                            "source_addrs": {
                                "terms": {
                                    "field": "src_ip.raw",
                                    "size": 0
                                },
                                "aggs": latestEventAggregationTemplate
                            }
                        }
                    }
                };
                return query;
            };

            service.archiveByQuery = function(options) {
                var options = options || {};
                var query = {
                    query: {
                        filtered: {
                            query: {
                                query_string: {
                                    query: options.query || "*"
                                }
                            },
                            filter: {
                                // As we are archiving, we are only working
                                // on events of type alert that are in the
                                // inbox.
                                and: [
                                    {exists: {field: "event_type"}},
                                    {term: {event_type: "alert"}},
                                    {term: {tags: "inbox"}}
                                ]
                            }
                        }
                    },
                    size: 1000,
                    fields: ["_index", "_type", "_id"]
                };

                // Add filters supplied in options.filters.
                if ("filters" in options) {
                    for (var i = 0; i < options.filters.length; i++) {
                        var filter = options.filters[i];
                        query.query.filtered.filter.and.push(filter);
                    }
                }

                // Add less than equals timestamp if options.lteTimestamp.
                if (options.lteTimestamp) {
                    query.query.filtered.filter.and.push({
                        range: {"@timestamp": {"lte": options.lteTimestamp}}
                    });
                }

                return $q(function(resolve, reject) {

                    (function execute() {
                        ElasticSearch.search(query).then(function(result) {
                            if (result.data.hits.hits.length == 0) {
                                resolve();
                                return;
                            }
                            ElasticSearch.bulkRemoveTag(result.data.hits.hits,
                                "inbox")
                                .finally(function() {
                                    execute();
                                })
                        }, function(result) {
                            reject(result);
                        });
                    })();

                });

            };

            service.getEvents = function(options) {

                if (options === undefined) {
                    options = {};
                }

                var query = buildEventQuery(options);

                return ElasticSearch.search(query).then(function(response) {

                    var result = {
                        total: response.data.hits.total,
                        events: response.data.hits.hits
                    };

                    // Augment the ES hit objects.
                    _.forEach(response.data.hits.hits, function(event) {
                        event.timestamp = event._source["@timestamp"];
                    });

                    return result;
                });

            };

            service.getAlertsGroupedBySignature = function(options) {
                if (options === undefined) {
                    options = {};
                }

                var query = buildAlertQueryGroupedBySignature(options);

                return ElasticSearch.search(query).then(function(response) {
                    var result = {
                        total: response.data.hits.total
                    };

                    result.events = _.map(response.data.aggregations.signature.buckets,
                        function(bucket) {
                            var latest = bucket.latest.hits.hits[0];
                            return {
                                signature: bucket.key,
                                count: bucket.doc_count,
                                latest: latest,
                                timestamp: latest._source["@timestamp"],
                                keys: {
                                    "alert.signature.raw": bucket.key
                                },
                                _source: latest._source
                            }
                        });
                    return result;
                })
            };

            service.getAlertsGroupedBySignatureAndSource = function(options) {
                if (options === undefined) {
                    options = {};
                }

                var query = buildAlertQueryGroupedBySignatureAndSource(options);

                return ElasticSearch.search(query).then(function(response) {
                    var result = {
                        total: response.data.hits.total,
                        events: []
                    };

                    _.forEach(response.data.aggregations.signature.buckets,
                        function(bucket0) {
                            var signature = bucket0.key;
                            _.forEach(bucket0.source_addrs.buckets,
                                function(bucket1) {
                                    var src_ip = bucket1.key;
                                    var latest = bucket1.latest.hits.hits[0];
                                    result.events.push({
                                        signature: signature,
                                        count: bucket1.doc_count,
                                        latest: latest,
                                        timestamp: latest._source["@timestamp"],
                                        keys: {
                                            "alert.signature.raw": signature,
                                            "src_ip.raw": src_ip
                                        },
                                        _source: latest._source
                                    });
                                });
                        });

                    return result;
                });
            };


            /**
             * Delete the provided event.
             *
             * @param event The event to delete.
             * @returns HttpPromise.
             */
            service.deleteEvent = function(event) {
                return ElasticSearch.delete(event._index, event._type,
                    event._id);
            };

            /**
             * Remove a tag from an event.
             *
             * @param event Event to remove tag from.
             * @param tag The tag to remove.
             * @returns HttpPromise.
             */
            service.removeTag = function(event, tag) {
                return ElasticSearch.removeTag(event, tag)
                    .success(function(response) {
                        _.remove(event._source.tags, function(t) {
                            return t === tag;
                        });
                    });
            };

            /**
             * Archive a single event.
             *
             * @param event The ES "hit" representing the event.
             */
            service.archiveEvent = function(event) {
                return ElasticSearch.removeTag(event, "inbox").then(
                    function(response) {
                    },
                    function(response) {
                        NotificationService.add("error",
                            printf("Failed to archive event."));
                    });
            };

            /**
             * Toggle a tag on event - remove it if it exists, otherwise add it.
             *
             * @param event Event to toggle tag on.
             * @param tag Tag to toggle.
             * @returns HttpPromise.
             */
            service.toggleTag = function(event, tag) {
                if (_.indexOf(event._source.tags, tag) > -1) {
                    return service.removeTag(event, tag);
                }
                else {
                    return ElasticSearch.addTag(event, tag).then(function() {
                        if (event._source.tags) {
                            event._source.tags.push(tag);
                        }
                        else {
                            event._source.tags = [tag];
                        }
                    });
                }
            };

            /**
             * Toggle the "starred" tag on an event.
             */
            service.toggleStar = function(event) {
                return service.toggleTag(event, "starred")
                    .then(function(response) {
                        console.log("Star toggled on event " + event._id);
                    }, function(response) {
                        console.log("Failed to toggle star on event " + event._id);
                        console.log(response);
                    });
            };

            return service;

        });

})();