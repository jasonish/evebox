/*
 * Copyright (c) 2014 Jason Ish
 * All rights reserved.
 */

/*
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

var app = angular.module("app", [
    "ngRoute", "ngResource", "ui.bootstrap", "ui.bootstrap.modal"]);

app.config(function ($routeProvider) {

    $routeProvider.when("/record/:id", {
        controller: "RecordController",
        templateUrl: "views/record.html"
    });

    $routeProvider.when("/:view", {
        controller: "AlertsController",
        templateUrl: "views/alerts.html"
    });

    $routeProvider.otherwise({redirectTo: "/inbox"});

});

app.controller('AlertsController', function (Keyboard, $route, $location,
    $timeout, $routeParams, $scope, $http, $filter, Config, ElasticSearch, Util,
    $modal, Cache) {

    // Debugging.
    scope = $scope;
    scope.Config = Config;
    scope.ElasticSearch = ElasticSearch;
    scope.filter = $filter;
    scope.$http = $http;
    scope.$routeParams = $routeParams;
    scope.Keyboard = Keyboard;
    scope.$location = $location;
    scope.$route = $route;
    scope.moment = moment;

    // Exports to scope.
    $scope.Util = Util;

    // Initial state.
    $scope.querySize = Config.elasticSearch.size;
    $scope.loading = false;
    $scope.state = "";
    $scope.errorMessage = "";
    $scope.activeRowIndex = 0;
    $scope.toJson = Util.toJson;
    $scope.view = $routeParams.view;

    // Use the provided aggregateBy, if none provided use the default as
    // provided by the user.
    if ("aggregateBy" in $routeParams) {
        $scope.aggregateBy = $routeParams.aggregateBy;
    }
    else if ($scope.view == "inbox") {
        $scope.aggregateBy = Config.defaultInboxAggregation || "";
    }
    else {
        $scope.aggregateBy = "";
    }

    $scope.sortBy = $routeParams.sortBy || "last";
    $scope.sortByOrder = $routeParams.sortByOrder || "desc";
    $scope.userQuery = $routeParams.q || "";
    $scope.page = $routeParams.page || 1;

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

    if ($routeParams.view == "starred") {
        $scope.filters.push({
            "term": {
                "tags": "starred"
            }
        });
    }

    $scope.displayErrorMessage = function (msg) {
        $scope.errorMessage = msg;
        $("#errorMessage").fadeIn();
        $("#errorMessage").delay(3000).fadeOut("slow");
    };

    $scope.toggleStar = function (item) {

        var tag = "starred";

        if (item._source.tags.indexOf(tag) == -1) {
            item._source.tags.push(tag);
            return ElasticSearch.addTag(item, tag);
        }
        else {
            item._source.tags = _.filter(item._source.tags, function (tag) {
                return tag != "starred";
            });
            return ElasticSearch.removeTag(item, tag);
        }
    };

    $scope.selectAll = function () {
        _.forEach($scope.hits.hits, function (hit) {
            hit.__selected = true;
        });
        $("button").blur();
    };

    $scope.deselectAll = function () {
        _.forEach($scope.hits.hits, function (hit) {
            hit.__selected = false;
        });
        $("button").blur();
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

    $scope.removeEvent = function (hit) {
        var activeItem = $scope.hits.hits[$scope.activeRowIndex];
        _.remove($scope.hits.hits, hit);
        // Update the currently selected item.
        var newIdx = $scope.hits.hits.indexOf(activeItem);
        if (newIdx >= 0) {
            $scope.activeRowIndex = newIdx;
        }
        else if ($scope.activeRowIndex >= $scope.hits.hits.length) {
            $scope.activeRowIndex = $scope.hits.hits.length - 1;
        }
    };

    $scope.archiveEvent = function (event) {
        ElasticSearch.removeTag(event, "inbox")
            .success(function () {
                $scope.removeEvent(event);
                if ($scope.hits.hits.length == 0) {
                    $scope.refresh();
                }
            });
    };

    $scope.archiveSelected = function () {
        if ($routeParams.view != "inbox") {
            return $scope.displayErrorMessage("Archive not valid in this context.");
        }

        var toArchive = _.filter($scope.response.hits.hits, function (hit) {
            return hit.__selected;
        });

        if (toArchive.length == 0) {
            return $scope.displayErrorMessage("No events selected.");
        }

        ElasticSearch.bulkRemoveTag(toArchive, "inbox")
            .success(function (response) {

                if (!response.errors) {
                    _.forEach(toArchive, $scope.removeEvent);
                }
                else {
                    /* There were errors. Only remove those that were archived
                     * and log an error for the events that errored out. */
                    var zipped = _.zip(response.items, toArchive);
                    _.forEach(zipped, function (item) {
                        var result = item[0];
                        var event = item[1];
                        if (result.update.status == 200) {
                            $scope.removeEvent(event);
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
        ElasticSearch.delete(event._index, event._type, event._id)
            .success(function () {
                $scope.removeEvent(event);

                if ($scope.hits.hits.length == 0) {
                    $scope.refresh();
                }

            });
    };

    $scope.deleteSelected = function () {
        var toDelete = _.filter($scope.hits.hits, function (hit) {
            return hit.__selected;
        });

        ElasticSearch.deleteEvents(toDelete)
            .success(function (response) {
                var zipped = _.zip(response.items, toDelete);
                _.forEach(zipped, function (item) {
                    var result = item[0];
                    var event = item[1];
                    if (result.delete.found) {
                        $scope.removeEvent(event);
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

    $scope.selectedCount = function () {
        try {
            return _.filter($scope.hits.hits, function (hit) {
                return hit.__selected;
            }).length;
        }
        catch (err) {
            return 0;
        }
    };

    /** Blur/unfocus an item by ID. */
    $scope.blurById = function (id) {
        $(id).blur();
    };

    /** Convert an alert severity into a Bootstrap class for colorization. */
    $scope.severityToBootstrapClass = function (severity) {
        switch (severity) {
            case 1:
                return "danger";
            case 2:
                return "warning";
            default:
                return "info";
        }
    };

    var setActiveAvent = function (event) {
        if (_.isNumber(event)) {
            $scope.activeRowIndex = event;
        }
    };

    var moveToNextEntry = function () {
        if ($scope.activeRowIndex + 1 < $scope.hits.hits.length) {
            $scope.activeRowIndex += 1;
            var element = $("#" + $scope.hits.hits[$scope.activeRowIndex]._id);
            Util.scrollElementIntoView(element);
        }
    };

    var moveToPreviousEntry = function () {
        if ($scope.activeRowIndex > 0) {
            $scope.activeRowIndex -= 1;
            if ($scope.activeRowIndex == 0) {
                $(window).scrollTop(0);
            }
            else {
                var element = $("#" + $scope.hits.hits[$scope.activeRowIndex]._id);
                Util.scrollElementIntoView(element);
            }
        }
    };

    /**
     * Refreshes the current search request to look for new events.
     */
    $scope.refresh = function () {
        $(".results").addClass("loading");
        $scope.submitSearchRequest();
    };

    /**
     * Called when the search form is submitted.
     *
     * Update the URL so the back-button works as expected.
     */
    $scope.onSearchFormSubmit = function () {
        var searchParams = {};

        if ($scope.userQuery) {
            searchParams.q = $scope.userQuery;
        }

        searchParams.aggregateBy = $scope.aggregateBy;

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
                                    query: $scope.userQuery || "*"
                                }
                            }
                        }
                    }
                }
            },
            size: $scope.querySize,
            from: Config.elasticSearch.size * ($scope.page - 1),
            sort: [
                {"@timestamp": {order: "desc"}}
            ]
        };

        request.query.filtered.filter = {
            "and": $scope.filters
        };

        if ($scope.aggregateBy == "signature+src") {
            delete(request.from);
            request.size = 0;
            request.aggs = {
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
                            "aggs": {
                                "last_timestamp": {
                                    "max": { "field": "@timestamp"}
                                }
                            }
                        }
                    }
                }
            }
        }
        else if ($scope.aggregateBy == "signature") {
            delete(request.from);
            request.size = 0;
            request.aggs = {
                "signature": {
                    "terms": {
                        "field": "alert.signature.raw",
                        "size": 0
                    },
                    "aggs": {
                        "last_timestamp": {
                            "max": { "field": "@timestamp"}
                        }
                    }
                }
            }
        }
        return request;
    };

    $scope.submitSearchRequest = function () {

        var request = $scope.createSearchRequest();

        $scope.loading = true;
        ElasticSearch.search(request).success(function (response) {
            $scope.handleSearchResponse(response);
            $(window).scrollTop(0);
        }).error(function (error) {
            if (error.status == 0) {
                $scope.displayErrorMessage(
                        "No response from Elastic Search at " + Config.elasticSearch.url);
            }
            else {
                $scope.displayErrorMessage(
                        "Error: " + error.status + " " + error.statusText);
            }
        }).finally(function () {
            $scope.loading = false;
        });
    };

    $scope.handleAggregateResponse = function (response) {

        $scope.aggregations = [];

        if ($scope.aggregateBy == "signature+src") {
            _.forEach(response.aggregations.signature.buckets, function (signature) {
                _.forEach(signature.source_addrs.buckets, function (addr) {
                    $scope.aggregations.push({
                        "signature": signature.key,
                        "last_timestamp": addr.last_timestamp.value,
                        "count": addr.doc_count,
                        "src_ip": addr.key
                    });
                });
            });
        }
        else if ($scope.aggregateBy == "signature") {
            _.forEach(response.aggregations.signature.buckets, function (signature) {
                $scope.aggregations.push({
                    "signature": signature.key,
                    "last_timestamp": signature.last_timestamp.value,
                    "count": signature.doc_count,
                });
            });
        }

        switch ($scope.sortBy) {
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
        if ($scope.sortByOrder == "desc") {
            $scope.aggregations = $scope.aggregations.reverse();
        }

        var severityCache = Cache.get("severityCache");

        // Resolve severity.
        _.forEach($scope.aggregations, function (agg) {

            if (agg.signature in severityCache) {
                agg.severity = severityCache[agg.signature];
            }
            else {

                var query = {
                    "query": {
                        "filtered": {
                            "filter": {
                                "and": [
                                    {
                                        "term": {
                                            "alert.signature.raw": agg.signature
                                        }
                                    },
                                    {
                                        "range": {
                                            "@timestamp": {
                                                "lte": agg.last_timestamp
                                            }
                                        }
                                    }
                                ]
                            }
                        }
                    },
                    "size": 1,
                    "sort": [
                        {
                            "@timestamp": {
                                "order": "desc"
                            }
                        }
                    ],
                    "fields": [
                        "alert.severity"
                    ]
                };

                if (agg.src_ip) {
                    query.query.filtered.filter.and.push({
                        "term": {
                            "src_ip.raw": agg.src_ip
                        }
                    });
                }

                !function (agg) {
                    ElasticSearch.search(query)
                        .success(function (response) {
                            if (response.hits.hits.length > 0) {
                                agg.severity = response.hits.hits[0].fields["alert.severity"][0];
                                severityCache[agg.signature] = agg.severity;
                            }
                        });
                }(agg);
            }
        });

        $(".results").removeClass("loading");
    };

    $scope.handleSearchResponse = function (response) {
        $scope.response = response;
        delete($scope.hits);
        delete($scope.buckets);
        $scope.activeRowIndex = 0;

        if ($scope.aggregateBy) {
            $scope.buckets = $scope.response.aggregations.signature.buckets;
            $scope.handleAggregateResponse(response);
            return;
        }

        $scope.hits = response.hits;

        // If no hits and we are not on page 1, decrement the page count
        // and try again.
        if ($scope.hits.hits.length == 0 && $scope.page > 1) {
            $scope.page--;
            $scope.refresh();
            return;
        }

        _.forEach($scope.hits.hits, function (hit) {
            hit._source["@timestamp"] =
                moment(hit._source["@timestamp"]).format();

            // Add a tags list if it doesn't exist.
            if (hit._source.tags == undefined) {
                hit._source.tags = [];
            }

        });

        $(".results").removeClass("loading");
    };

    $scope.renderIpAddress = function (addr) {
        if (addr === undefined) {
            return "";
        }
        addr = addr.replace(/0000/g, "");
        while (addr.indexOf(":0:") > -1) {
            addr = addr.replace(/:0:/g, "::");
        }
        addr = addr.replace(/:::+/g, "::");
        return addr;
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
            $scope.displayErrorMessage("No events to archive.");
            return;
        }

        var lastTimestamp = $scope.hits.hits[0]._source["@timestamp"];
        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.userQuery || "*"
                        }
                    },
                    filter: {
                        and: [
                            {
                                term: { tags: "inbox" }
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
            $scope.displayErrorMessage("No events to delete.");
            return;
        }

        var latestTimestamp = $scope.hits.hits[0]._source["@timestamp"];

        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.userQuery || "*"
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

    var toggleSelected = function () {
        var event = $scope.hits.hits[$scope.activeRowIndex];
        event.__selected = !event.__selected;
    };

    $scope.gotoPage = function (what) {

        var last = Math.floor($scope.hits.total / $scope.querySize) + 1;

        switch (what) {
            case "first":
                $scope.page = 1;
                break;
            case "prev":
                if ($scope.page > 1) {
                    $scope.page--;
                }
                break;
            case "next":
                if ($scope.page < last) {
                    $scope.page++;
                }
                break;
            case "last":
                $scope.page = last;
                break;
        }
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

    Keyboard.scopeBind($scope, "/", function (e) {
        e.preventDefault();
        $("#query-input").focus();
        $("#query-input").select();
    });

    Keyboard.scopeBind($scope, "^", function () {
        $("#aggregate-by-input").focus();
    });

    Keyboard.scopeBind($scope, "j", function (e) {
        $scope.$apply(function () {
            moveToNextEntry();
        });
    });

    Keyboard.scopeBind($scope, "shift+j", function (e) {
        $scope.$apply(function () {
            toggleSelected();
            moveToNextEntry();
        });
    });

    Keyboard.scopeBind($scope, "k", function (e) {
        $scope.$apply(function () {
            moveToPreviousEntry();
        });
    });

    Keyboard.scopeBind($scope, "shift+k", function (e) {
        $scope.$apply(function () {
            toggleSelected();
            moveToPreviousEntry();
        });
    });

    Keyboard.scopeBind($scope, "x", function (e) {
        $scope.$apply(function () {
            toggleSelected();
        });
    });

    Keyboard.scopeBind($scope, "s", function (e) {
        $scope.$apply(function () {
            $scope.toggleStar($scope.hits.hits[$scope.activeRowIndex]);
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
            $scope.toggleOpenEvent($scope.hits.hits[$scope.activeRowIndex]);
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

    Keyboard.scopeBind($scope, ">", function (e) {
        $scope.$apply(function () {
            $scope.gotoPage("next");
        });
    });

    Keyboard.scopeBind($scope, "<", function (e) {
        $scope.$apply(function () {
            $scope.gotoPage("prev");
        });
    });

    Keyboard.scopeBind($scope, "H", function (e) {
        $scope.$apply(function () {
            $(window).scrollTop(0);
            setActiveAvent(0);
        });
    });

    Keyboard.scopeBind($scope, "G", function (e) {
        $scope.$apply(function () {
            $(window).scrollTop($(document).height())
            setActiveAvent($scope.hits.hits.length - 1);
        });
    });

    Keyboard.scopeBind($scope, ".", function (e) {
        $(".dropdown-toggle.keyboard").first().dropdown("toggle");
    });

    $scope.submitSearchRequest();
})
;
