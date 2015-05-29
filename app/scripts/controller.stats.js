'use strict';

(function() {

    angular.module("app").controller("StatsController", StatsController);

    function StatsController($http, ElasticSearch) {

        var vm = this;

        vm.duration = "24h";
        vm.interval = "15m";

        var parseDuration = function(duration) {
            var match = duration.match(/(\d+)(.*)/);
            var value = parseInt(match[1]);
            var unit = match[2];
            return moment.duration(value, unit);
        };

        /**
         * Return histograms for each type of EVE record.
         */
        var refreshRecordTypeCounts = function(rangeFilter) {
            var duration = parseDuration(vm.duration);
            var interval = vm.interval;

            return ElasticSearch.search({
                query: {
                    filtered: {
                        filter: {
                            and: [
                                {
                                    exists: {field: "event_type"}
                                },
                                rangeFilter
                            ]
                        }
                    }

                },
                size: 0,
                aggregations: {
                    types: {
                        terms: {
                            field: "event_type",
                            size: 0
                        },
                        aggregations: {
                            counts: {
                                date_histogram: {
                                    field: "@timestamp",
                                    interval: interval,
                                    min_doc_count: 0
                                }
                            }
                        }
                    }
                }
            }).then(function(response) {
                var data = {};

                _.forEach(response.data.aggregations.types.buckets,
                    function(bucket) {
                        var event_type = bucket.key;
                        data[event_type] = [];
                        _.forEach(bucket.counts.buckets, function(bucket) {
                            data[event_type].push({
                                date: moment(bucket.key).toDate(),
                                value: bucket.doc_count
                            });
                        })
                    });

                return data;
            });
        };

        var refresh = function() {

            var duration = parseDuration(vm.duration);
            var interval = vm.interval;

            var rangeFilter = {
                range: {
                    "@timestamp": {
                        "gte": moment().subtract(duration)
                    }
                }
            };

            refreshRecordTypeCounts(rangeFilter).then(function(counts) {

                for (var recordType in counts) {
                    var div = document.createElement("div");
                    div.setAttribute("id", recordType);
                    document.getElementById("counts-container").appendChild(div);

                    MG.data_graphic({
                        title: recordType.toUpperCase(),
                        full_width: true,
                        height: 300,
                        target: "#" + recordType,
                        data: counts[recordType],
                        chart_type: "histogram",
                        binned: true
                    });

                }

            });

        };

        vm.refresh = refresh;

        // Init.
        refresh();

    }
})();