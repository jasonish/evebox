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

(function () {

    angular.module("app").controller("AlertsController", AlertsController);

    function AlertsController(Keyboard, $route, $location,
        $timeout, $routeParams, $scope, $http, $filter, Config, ElasticSearch,
        Util, $modal, Cache, EventRepository, NotificationMessageService) {

        var mv = this;

        // Debugging.
        $scope.Config = Config;
        $scope.ElasticSearch = ElasticSearch;
        $scope.filter = $filter;
        $scope.$http = $http;
        $scope.$routeParams = $routeParams;
        $scope.Keyboard = Keyboard;
        $scope.$location = $location;
        $scope.$route = $route;
        $scope.moment = moment;

        // Exports to scope.
        $scope.Util = Util;

        // Initial state.
        $scope.querySize = Config.elasticSearch.size;
        $scope.loading = false;
        $scope.view = $routeParams.view;

        /* Model for search form.  Also includes parameters not available in the
         * search form, but still used to build the query. */
        $scope.searchModel = {
            userQuery: $routeParams.q || "",
            aggregateBy: (function () {
                if ("aggregateBy" in $routeParams) {
                    return $routeParams.aggregateBy;
                }
                else if ($scope.view == "inbox") {
                    return Config.defaultInboxAggregation || "";
                }
                else {
                    return "";
                }
            })(),
            sortBy: $routeParams.sortBy || "last",
            sortByOrder: $routeParams.sortByOrder || "desc",
            page: $routeParams.page || 1
        };

        /* Model containing normalized result data. */
        $scope.resultsModel = {
            rows: [],
            activeRowIndex: 0
        };

        // Setup the search filters.
        $scope.filters = [
            {
                "match_all": {}
            }
        ];

        $scope.filters.push({
            "term": {
                "event_type": "alert"
            }
        });

        if ($routeParams.view == "inbox") {
            $scope.filters.push({
                "term": {
                    "tags": "inbox"
                }
            });
        }
        else if ($routeParams.view == "starred") {
            $scope.filters.push({
                "term": {
                    "tags": "starred"
                }
            });
        }

        // Model for search form aggregation options.
        $scope.aggregationOptions = [
            {
                name: "",
                value: ""
            },
            {
                name: "Signature",
                value: "signature"
            },
            {
                name: "Signature+Source",
                value: "signature+src"
            }
        ];

        $scope.toggleStar = function (event) {
            EventRepository.toggleStar(event);
        };

        $scope.selectAll = function () {
            _.forEach($scope.resultsModel.rows, function (row) {
                row.__selected = true;
            });
        };

        $scope.deselectAll = function () {
            _.forEach($scope.resultsModel.rows, function (row) {
                row.__selected = false;
            });
        };

        $scope.toggleOpenEvent = function (event) {

            /* Close all other events. */
            _.forEach($scope.response.hits.hits, function (hit) {
                if (hit != event) {
                    hit.__open = false;
                }
            });

            event.__open = !event.__open;

            if (event.__open) {
                // If open, do the scroll in a timeout as it has to be done after
                // apply.
                if (event.__open) {
                    $timeout(function () {
                        $(window).scrollTop($("#" + event._id).offset().top);
                    }, 0);
                }
            }
        };

        /**
         * Remove a row from the model.
         */
        $scope.removeRow = function (row) {
            var rowIndex = _.indexOf($scope.resultsModel.rows, row);
            if ($scope.resultsModel.activeRowIndex > rowIndex) {
                $scope.resultsModel.activeRowIndex--;
            }
            else if ($scope.resultsModel.activeRowIndex == $scope.resultsModel.rows.length - 1) {
                $scope.resultsModel.activeRowIndex--;
            }
            _.remove($scope.resultsModel.rows, row);
        };

        $scope.archiveEvent = function (event) {
            EventRepository.removeTag(event, "inbox")
                .success(function () {
                    $scope.removeRow(event);
                    if ($scope.hits.hits.length == 0) {
                        $scope.refresh();
                    }
                });
        };

        $scope.archiveSelected = function () {

            if ($routeParams.view != "inbox") {
                return NotificationMessageService.add("warning", "Archive not valid in this context");
            }

            var toArchive = $scope.getSelectedRows();

            if (toArchive.length == 0) {
                return NotificationMessageService.add("warning", "No events selected.");
            }

            ElasticSearch.bulkRemoveTag(toArchive, "inbox")
                .success(function (response) {

                    if (!response.errors) {
                        _.forEach(toArchive, $scope.removeRow);
                    }
                    else {
                        /* There were errors. Only remove those that were archived
                         * and log an error for the events that errored out. */
                        var zipped = _.zip(response.items, toArchive);
                        _.forEach(zipped, function (item) {
                            var result = item[0];
                            var event = item[1];
                            if (result.update.status == 200) {
                                $scope.removeRow(event);
                            }
                            else {
                                /* TODO: Make user visible. */
                                console.log(Util.formatString("Failed to delete event {0}: {1}",
                                    result.update._id, result.update.status));
                            }
                        });
                    }

                    if ($scope.hits.hits.length == 0) {
                        $scope.refresh();
                    }

                })
                .error(function (error) {
                    console.log(error);
                });

        };

        $scope.deleteEvent = function (event) {
            EventRepository.deleteEvent(event)
                .success(function () {
                    $scope.removeRow(event);

                    if ($scope.hits.hits.length == 0) {
                        $scope.refresh();
                    }
                });
        };

        $scope.getSelectedRows = function () {
            return _.filter($scope.resultsModel.rows, function (row) {
                return row.__selected;
            });
        };

        $scope.selectedCount = function () {
            return $scope.getSelectedRows().length;
        };

        $scope.deleteSelected = function () {
            var toDelete = $scope.getSelectedRows();

            ElasticSearch.deleteEvents(toDelete)
                .success(function (response) {
                    var zipped = _.zip(response.items, toDelete);
                    _.forEach(zipped, function (item) {
                        var result = item[0];
                        var event = item[1];
                        if (result.delete.found) {
                            $scope.removeRow(event);
                        }
                        else {
                            /* TODO: Make user visible. */
                            console.log(Util.formatString("Failed to delete event {0}: {1}",
                                result.delete._id, result.delete.status));
                        }
                    });

                    if ($scope.hits.hits.length == 0) {
                        $scope.refresh();
                    }
                })
                .error(function (error) {
                    console.log(error);
                });
        };

        /**
         * Refreshes the current search request to look for new events.
         */
        $scope.refresh = function () {
            $scope.submitSearchRequest();
        };

        /**
         * Called when the search form is submitted.
         *
         * Update the URL so the back-button works as expected.
         */
        $scope.onSearchFormSubmit = function () {

            var searchParams = {};

            if ($scope.searchModel.userQuery) {
                searchParams.q = $scope.searchModel.userQuery;
            }

            searchParams.aggregateBy = $scope.searchModel.aggregateBy;

            $location.search(searchParams);
        };

        $scope.createSearchRequest = function () {
            var request = {
                query: {
                    filtered: {
                        query: {
                            bool: {
                                must: {
                                    query_string: {
                                        query: $scope.searchModel.userQuery || "*"
                                    }
                                }
                            }
                        }
                    }
                },
                size: $scope.querySize,
                from: Config.elasticSearch.size * ($scope.searchModel.page - 1),
                sort: [
                    {"@timestamp": {order: "desc"}}
                ]
            };

            request.query.filtered.filter = {
                "and": $scope.filters
            };

            if ($scope.searchModel.aggregateBy == "signature+src") {
                delete(request.from);
                request.size = 0;
                request.aggs = EventRepository.aggregateBySignatureSrc;
            }
            else if ($scope.searchModel.aggregateBy == "signature") {
                delete(request.from);
                request.size = 0;
                request.aggs = EventRepository.aggregateBySignature;
            }
            return request;
        };

        $scope.submitSearchRequest = function () {

            var request = $scope.createSearchRequest();

            $scope.loading = true;
            ElasticSearch.search(request).success(function (response) {
                $scope.response = response;
                delete($scope.hits);
                delete($scope.buckets);
                $scope.resultsModel.activeRowIndex = 0;
                if (request.aggs) {
                    $scope.handleAggregateResponse(response);
                }
                else {
                    $scope.handleSearchResponse(response);
                }
                $(window).scrollTop(0);
            }).error(function (error) {
                if (error.status == 0) {
                    NotificationMessageService.add("danger",
                        "No response from Elastic Search at " + Config.elasticSearch.url);
                }
                else {
                    NotificationMessageService.add("danger",
                        "Error: " + error.status + " " + error.statusText);
                }
            }).finally(function () {
                $scope.loading = false;
            });
        };

        $scope.handleAggregateResponse = function (response) {

            $scope.aggregations = [];

            if ($scope.searchModel.aggregateBy == "signature+src") {
                _.forEach(response.aggregations.signature.buckets, function (signature) {
                    _.forEach(signature.source_addrs.buckets, function (addr) {
                        $scope.aggregations.push({
                            "signature": signature.key,
                            "last_timestamp": addr.latest_event.hits.hits[0]._source["@timestamp"],
                            "severity": addr.latest_event.hits.hits[0]._source.alert.severity,
                            "category": addr.latest_event.hits.hits[0]._source.alert.category,
                            "count": addr.doc_count,
                            "src_ip": addr.key
                        });
                    });
                });
            }
            else if ($scope.searchModel.aggregateBy == "signature") {
                _.forEach(response.aggregations.signature.buckets, function (signature) {
                    $scope.aggregations.push({
                        "signature": signature.key,
                        "last_timestamp": signature.latest_event.hits.hits[0]._source["@timestamp"],
                        "category": signature.latest_event.hits.hits[0]._source.alert.category,
                        "severity": signature.latest_event.hits.hits[0]._source.alert.severity,
                        "count": signature.doc_count
                    });
                });
            }

            switch ($scope.searchModel.sortBy) {
                case "last":
                    $scope.aggregations = _.sortBy($scope.aggregations, function (agg) {
                        return agg.last_timestamp;
                    });
                    break;
                case "count":
                    $scope.aggregations = _.sortBy($scope.aggregations, function (agg) {
                        return agg.count;
                    });
                    break;
                case "message":
                    $scope.aggregations = _.sortBy($scope.aggregations, function (agg) {
                        return agg.signature;
                    });
                    break;
                case "src_ip":
                    $scope.aggregations = _.sortBy($scope.aggregations, function (agg) {
                        return agg.src_ip;
                    });
                    break;
            }
            if ($scope.searchModel.sortByOrder == "desc") {
                $scope.aggregations = $scope.aggregations.reverse();
            }

            $scope.resultsModel.rows = $scope.aggregations;

            $(".results").removeClass("loading");
        };

        $scope.handleSearchResponse = function (response) {
            $scope.resultsModel.rows = response.hits.hits;

            $scope.hits = response.hits;

            // If no hits and we are not on page 1, decrement the page count
            // and try again.
            if ($scope.hits.hits.length == 0 && $scope.page > 1) {
                $scope.page--;
                $scope.refresh();
                return;
            }

            _.forEach($scope.hits.hits, function (hit) {
                // Add a tags list if it doesn't exist.
                if (hit._source.tags == undefined) {
                    hit._source.tags = [];
                }

            });

            // Cache events.
            var eventCache = Cache.get("events");
            _.forEach($scope.hits.hits, function (hit) {
                eventCache[hit._id] = hit;
            });

            $(".results").removeClass("loading");
        };

        $scope.doArchiveByQuery = function (title, query) {

            var jobs = [
                {
                    label: title,
                    query: query
                }
            ];

            var modal = $modal.open({
                templateUrl: "templates/modal-progress.html",
                controller: "ModalProgressController",
                resolve: {
                    jobs: function () {
                        return jobs;
                    }
                }
            });

            var doArchiveJob = function (job) {

                ElasticSearch.search(job.query)
                    .success(function (response) {
                        if (job.max === undefined) {
                            job.max = response.hits.total;
                            job.value = 0;
                        }
                        if (response.hits.hits.length > 0) {
                            ElasticSearch.bulkRemoveTag(response.hits.hits, "inbox")
                                .success(function (response) {
                                    job.value += response.items.length;
                                    doArchiveJob(job);
                                });
                        }
                        else {
                            _.remove(jobs, job);
                            if (jobs.length == 0) {
                                modal.close();
                                $scope.page = 1;
                                $scope.refresh();
                            }
                        }
                    });

            };

            _.forEach(jobs, doArchiveJob);
        };

        $scope.archiveByQuery = function () {
            if ($scope.response.hits.total == 0) {
                NotificationMessageService.add("warning", "No events to archive.");
                return;
            }

            var lastTimestamp = $scope.hits.hits[0]._source["@timestamp"];
            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        },
                        filter: {
                            and: [
                                {
                                    term: {tags: "inbox"}
                                },
                                {
                                    range: {
                                        "@timestamp": {
                                            "lte": lastTimestamp
                                        }
                                    }
                                }
                            ]
                        }
                    }
                },
                size: 1000,
                fields: ["_index", "_type", "_id"],
                sort: [
                    {"@timestamp": {order: "desc"}}
                ]
            };

            $scope.doArchiveByQuery("Archiving...", query);
        };

        $scope.deleteByQuery = function () {
            if ($scope.response.hits.total == 0) {
                NotificationMessageService.add("warning", "No events to delete.");
                return;
            }

            var latestTimestamp = $scope.hits.hits[0]._source["@timestamp"];

            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        }
                    }
                }
            };

            query.query.filtered.filter = {
                "and": _.cloneDeep($scope.filters)
            };

            query.query.filtered.filter.and.push({
                "range": {
                    "@timestamp": {
                        "lte": latestTimestamp
                    }
                }
            });

            ElasticSearch.deleteByQuery(query)
                .success(function (response) {
                    $scope.page = 1;
                    $scope.refresh();
                })
                .error(function (error) {
                    console.log(error);
                })
        };

        $scope.getActiveRow = function () {
            return $scope.resultsModel.rows[$scope.resultsModel.activeRowIndex];
        };

        $scope.toggleActiveRowCheckbox = function () {
            var row = $scope.getActiveRow();
            row.__selected = !row.__selected;
        };

        $scope.gotoPage = function (page) {
            $scope.page = page;
            $location.search("page", $scope.page);
        };

        /*
         * Keyboard bindings.
         */

        $scope.$on("$destroy", function () {
            Keyboard.resetScope($scope);
        });

        Keyboard.scopeBind($scope, "+", function () {
            $scope.$apply(function () {
                $scope.increaseRequestSize();
            });
        });

        Keyboard.scopeBind($scope, "-", function () {
            $scope.$apply(function () {
                $scope.decreaseRequestSize();
            });
        });

        Keyboard.scopeBind($scope, "r", function (e) {
            $scope.$apply(function () {
                $scope.refresh();
            })
        });

        Keyboard.scopeBind($scope, "^", function () {
            $("#aggregate-by-input").focus();
        });

        Keyboard.scopeBind($scope, "shift+j", function (e) {
            $scope.toggleActiveRowCheckbox();
            Mousetrap.trigger("j");
        });

        Keyboard.scopeBind($scope, "shift+k", function (e) {
            $scope.toggleActiveRowCheckbox();
            Mousetrap.trigger("k");
        });

        Keyboard.scopeBind($scope, "x", function (e) {
            $scope.$apply(function () {
                $scope.toggleActiveRowCheckbox();
            });
        });

        Keyboard.scopeBind($scope, "s", function (e) {
            $scope.$apply(function () {
                $scope.toggleStar($scope.hits.hits[$scope.resultsModel.activeRowIndex]);
            });
        });

        Keyboard.scopeBind($scope, "* a", function (e) {
            $scope.$apply(function () {
                $scope.selectAll()
            });
        });

        Keyboard.scopeBind($scope, "* n", function (e) {
            $scope.$apply(function () {
                $scope.deselectAll()
            });
        });

        Keyboard.scopeBind($scope, "o", function (e) {
            $scope.$apply(function () {
                $scope.toggleOpenEvent($scope.hits.hits[$scope.resultsModel.activeRowIndex]);
            });
        });

        Keyboard.scopeBind($scope, "O", function (e) {
            $scope.$apply(function () {
                window.location = "#/record/" +
                $scope.hits.hits[$scope.resultsModel.activeRowIndex]._id;
            });
        });

        Keyboard.scopeBind($scope, "e", function (e) {
            $scope.$apply(function () {
                $scope.archiveSelected();
            });
        });

        Keyboard.scopeBind($scope, "#", function (e) {
            $scope.$apply(function () {
                $scope.deleteSelected()
            });
        });

        Keyboard.scopeBind($scope, ".", function (e) {
            $(".dropdown-toggle.keyboard").first().dropdown("toggle");
        });

        $scope.submitSearchRequest();
    };

})();

