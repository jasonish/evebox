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

app.controller("NavBarController", function ($routeParams, $scope, $modal,
    $location, Keyboard, EventRepository, $timeout) {

    $scope.$routeParams = $routeParams;

    $scope.openConfig = function () {
        $modal.open({
            templateUrl: "templates/config.html",
            controller: "ConfigController"
        });
    };

    $scope.openHelp = function () {
        $modal.open({
            templateUrl: "templates/help.html",
            size: "lg"
        });
    };

    Keyboard.scopeBind($scope, "g i", function (e) {
        $scope.$apply(function () {
            $location.url("/inbox");
        });
    });

    Keyboard.scopeBind($scope, "g s", function (e) {
        $scope.$apply(function () {
            $location.url("/starred")
        });
    });

    Keyboard.scopeBind($scope, "g a", function (e) {
        $scope.$apply(function () {
            $location.url("/alerts");
        });
    });

    Keyboard.scopeBind($scope, "g o", function () {
        $scope.$apply(function () {
            $("#other-menu-dropdown-toggle").dropdown('toggle');
        });
    });

    Keyboard.scopeBind($scope, "g c", function (e) {
        $scope.openConfig();
    });

    Keyboard.scopeBind($scope, "?", function (e) {
        $scope.openHelp();
    });
});

app.controller("ConfigController", function ($scope, $modalInstance, Config) {

    $scope.config = Config;

    $scope.ok = function () {
        Config.save();
        $modalInstance.close();
    };

    $scope.cancel = function () {
        $modalInstance.dismiss();
    };

});

app.controller("RecordController", function ($scope, $routeParams, Util,
    ElasticSearch, Config) {

    // Export some functions to $scope.
    $scope.Util = Util;
    $scope.Config = Config;

    ElasticSearch.searchEventById($routeParams.id)
        .success(function (response) {
            $scope.response = response;
            $scope.hits = response.hits;

            _.forEach($scope.hits.hits, function (hit) {

                if (hit._source.alert) {
                    hit.__title = hit._source.alert.signature;
                    hit.__titleClass = Util.severityToBootstrapClass(hit._source.alert.severity, "alert-");
                }
                else if (hit._source.dns) {
                    hit.__title = hit._source.event_type.toUpperCase() + ": " +
                        hit._source.dns.rrname;
                    hit.__titleClass = "alert-info";
                }
                else if (hit._source.tls) {
                    hit.__title = hit._source.event_type.toUpperCase() + ": " +
                        hit._source.tls.subject;
                    hit.__titleClass = "alert-info";
                }
                else if (hit._source.http) {
                    hit.__title = hit._source.event_type.toUpperCase() + ": " +
                        hit._source.http.http_method + " " +
                        hit._source.http.hostname;
                }
                else {
                    hit.__title = hit._source.event_type.toUpperCase();
                    hit.__titleClass = "alert-info";
                }

                if (!hit.__titleClass) {
                    hit.__titleClass = "alert-info";
                }

            });

        });

});

app.controller("EventDetailController", function ($scope, Keyboard, Config,
    ElasticSearch, EventRepository, Util) {

    $scope.Config = Config;

    /* Suricata can store the payload as base64 or printable.  Attempt to
     * guess which it is here. */
    $scope.payloadIsBase64 = Util.isBase64($scope.hit._source.payload);

    $scope.b64ToText = function (data) {
        return atob(data);
    };

//    $scope.toggleCollapse = function (elementId) {
//        if ($scope.isCollapsed(elementId)) {
//            $(elementId).addClass("in");
//        }
//        else {
//            $(elementId).removeClass("in");
//        }
//    };
//
//    $scope.isCollapsed = function (elementId) {
//        return !$(elementId).hasClass("in");
//    };
//
    $scope.b64ToHex = function (data) {
        var hex = Util.base64ToHexArray(data);
        var buf = "";
        for (var i = 0; i < hex.length; i++) {
            if (i > 0 && i % 16 == 0) {
                buf += "\n";
            }
            buf += hex[i] + " ";
        }
        return buf;
    };

    $scope.archiveEvent = function (event) {
        if ($scope.$parent.archiveEvent === undefined) {
            ElasticSearch.removeTag(event, "inbox")
                .success(function (response) {
                    _.remove(event._source.tags, function (tag) {
                        return tag == "inbox";
                    })
                });
        }
        else {
            $scope.$parent.archiveEvent(event);
        }
    };

    $scope.deleteEvent = function (event) {
        if ($scope.$parent.deleteEvent === undefined) {
            EventRepository.deleteEvent(event)
                .success(function (response) {
                    $scope.$emit("eventDeleted", event);
                });
        }
        else {
            $scope.$parent.deleteEvent(event);
        }
    };

    $scope.toggleStar = function (event) {
        EventRepository.toggleStar(event);
    };

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, ".", function () {
        $("#event-detail-more-button").dropdown('toggle');
    });

});

app.controller("ModalProgressController", function ($scope, jobs) {
    $scope.jobs = jobs;
});

/**
 * Controller for the all events view.
 */
app.controller("EventsController", function ($scope, Util, Keyboard, Config,
    ElasticSearch, $routeParams, $location, $sce) {

    EventsController = $scope;

    $scope.$routeParams = $routeParams;
    $scope.Util = Util;
    $scope.page = $routeParams.page || 1;
    $scope.querySize = Config.elasticSearch.size;

    $scope.searchModel = {
        userQuery: $routeParams.q || ""
    };

    $scope.filters = [
        {
            "exists": { "field": "event_type" }
        }
    ];

    $scope.resultsModel = {
        rows: [],
        activeRowIndex: 0
    };

    $scope.eventMessage = function (event) {
        switch (event.event_type) {
            default:
                var parts = [];
                _.forIn(event[event.event_type], function (value, key) {
                    parts.push('<span style="color: #808080;">' +
                        key +
                        ':</span> ' +
                        '<span style="word-break: break-all;">' +
                        value +
                        '</span>');
                });
                var msg = parts.join("; ");
                return $sce.trustAsHtml(msg);
        }
    };

    $scope.submitSearchRequest = function (request) {
        $scope.searchRequest = request;
        $scope.loading = true;
        ElasticSearch.search(request)
            .success($scope.onSearchResponseSuccess)
            .error(function (error) {
            })
            .finally(function () {
                $scope.loading = false;
            });
    };

    $scope.onSearchResponseSuccess = function (response) {
        $scope.response = response;

        $scope.resultsModel.rows = response.hits.hits.map(function (hit) {

            // If row is an alert, set the class based on the severity.
            if (hit._source.alert) {
                var trClass = [Util.severityToBootstrapClass(
                    hit._source.alert.severity)];
            }
            else {
                var trClass = ["info"];
            }

            return {
                trClass: trClass,

                source: hit,

                timestamp: moment(hit._source["@timestamp"]).format("YYYY-MM-DD HH:mm:ss.SSS"),
                src_ip: Util.printIpAddress(hit._source.src_ip),
                dest_ip: Util.printIpAddress(hit._source.dest_ip),
                message: $scope.eventMessage(hit._source),
                event_type: hit._source.event_type
            };
        });

        $scope.resultsModel.rows = _.sortBy($scope.resultsModel.rows, function (row) {
            return Util.timestampToFloat(row.source._source.timestamp);
        }).reverse();
    };

    $scope.toggleOpenEvent = function (event) {
        _.forEach($scope.resultsModel.rows, function (row) {
            if (row != event) {
                row.__open = false;
            }
        });
        event.__open = !event.__open;
    };

    $scope.refresh = function () {
        $scope.submitSearchRequest($scope.searchRequest);
    };

    $scope.gotoPage = function (page) {
        $scope.page = page;
        $location.search("page", $scope.page);
    };

    $scope.doSearch = function () {

        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.searchModel.userQuery || "*"
                        }
                    },
                    filter: {
                        and: _.cloneDeep($scope.filters)
                    }
                }
            },
            size: $scope.querySize || 100,
            from: Config.elasticSearch.size * (($scope.page || 1) - 1),
            sort: [
                {"@timestamp": {order: "desc"}}
            ],
            timeout: 6000
        };

        $scope.submitSearchRequest(query);
    };

    $scope.onSearchFormSubmit = function () {
        $location.search("q", $scope.searchModel.userQuery);
    };

    $scope.$on("eventDeleted", function (e, event) {
        _.remove($scope.resultsModel.rows, function (row) {
            return row.source === event;
        });
    });

    $scope.getActiveRow = function () {
        return $scope.resultsModel.rows[$scope.resultsModel.activeRowIndex];
    };

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, "o", function () {
        $scope.$apply(function () {
            $scope.toggleOpenEvent($scope.getActiveRow());
        });
    });

    Keyboard.scopeBind($scope, "r", function () {
        $scope.$apply(function () {
            $scope.refresh();
        });
    });

    $scope.doSearch();
});

/**
 * Controller for the alert-only views (inbox, starred, alerts).
 */
app.controller('AlertsController', function (Keyboard, $route, $location,
    $timeout, $routeParams, $scope, $http, $filter, Config, ElasticSearch, Util,
    $modal, Cache, EventRepository, NotificationMessageService) {

    // Debugging.
    scope = $scope;
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
            from: Config.elasticSearch.size * ($scope.page - 1),
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
        else if ($scope.searchModel.aggregateBy == "signature") {
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
                        "last_timestamp": addr.last_timestamp.value,
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
                    "last_timestamp": signature.last_timestamp.value,
                    "count": signature.doc_count,
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
        $scope.resultsModel.activeRowIndex = 0;

        if ($scope.searchModel.aggregateBy) {
            $scope.buckets = $scope.response.aggregations.signature.buckets;
            $scope.handleAggregateResponse(response);
            return;
        }

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
            hit._source["@timestamp"] =
                moment(hit._source["@timestamp"]).format();

            // Add a tags list if it doesn't exist.
            if (hit._source.tags == undefined) {
                hit._source.tags = [];
            }

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
});

app.controller("AggregatedAlertsController", function ($scope, $location,
    Keyboard, ElasticSearch, $modal, $routeParams, NotificationMessageService,
    Util) {

    AggregatedAlertsController = $scope;

    /**
     * Opens an aggregation in "flat" view by explicitly setting no aggregation
     * and constructing a query.
     */
    $scope.openAggregation = function (agg) {
        var searchParams = $location.search();
        searchParams.aggregateBy = "";
        searchParams.q = "q" in searchParams ? searchParams.q : "";
        searchParams.q += " +alert.signature.raw:\"" + agg.signature + "\"";
        if (agg.src_ip) {
            searchParams.q += " +src_ip.raw:\"" + agg.src_ip + "\"";
        }
        searchParams.q = searchParams.q.trim();

        $location.search(searchParams);
    };

    $scope.totalEventCount = function () {
        return _.reduce($scope.aggregations, function (sum, agg) {
            return sum + agg.count;
        }, 0);
    };

    $scope.deleteRow = function (row) {
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
                    "lte": row.last_timestamp
                }
            }
        });

        query.query.filtered.filter.and.push({
            "term": {
                "alert.signature.raw": row.signature
            }
        });

        if (row.src_ip) {
            query.query.filtered.filter.and.push({
                "term": {
                    "src_ip.raw": row.src_ip
                }
            });
        }

        return ElasticSearch.deleteByQuery(query);

    };

    $scope.deleteSelected = function () {
        var selectedRows = $scope.getSelectedRows();

        if (!selectedRows) {
            return;
        }

        var row = selectedRows.pop();

        $scope.deleteRow(row)
            .success(function (response) {
                $scope.removeRow(row);
                NotificationMessageService.add("info", Util.formatString("Deleted: {0}", row.signature));
            })
            .finally(function () {
                $scope.deleteSelected();
            });
    };

    $scope.buildArchiveRowQuery = function (row) {
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
                                term: { tags: "inbox"}
                            },
                            {
                                range: {
                                    "@timestamp": {
                                        "lte": row.last_timestamp
                                    }
                                }
                            },
                            {
                                term: {
                                    "alert.signature.raw": row.signature
                                }
                            }
                        ]
                    }
                }
            },
            size: 1000,
            fields: []
        };

        if (row.src_ip) {
            query.query.filtered.filter.and.push({
                "term": {
                    "src_ip.raw": row.src_ip
                }
            });
        }

        return query;
    };

    $scope.archiveSelected = function () {

        if ($scope.$routeParams.view != "inbox") {
            return NotificationMessageService.add("warning", "Archive not valid in this context.");
        }

        var selectedRows = $scope.getSelectedRows();

        if (selectedRows.length == 0) {
            return NotificationMessageService.add("warning", "No events selected.");
        }

        var archiveJobs = selectedRows.map(function (row) {
            return {
                row: row,
                label: row.signature,
                query: $scope.buildArchiveRowQuery(row)
            };
        });

        var modal = $modal.open({
            templateUrl: "templates/modal-progress.html",
            controller: "ModalProgressController",
            resolve: {
                jobs: function () {
                    return archiveJobs;
                }
            }
        });

        modal.result.then(function () {
            if ($scope.aggregations.length == 0) {
                $scope.refresh();
            }
        });

        var doArchiveJob = function () {

            if (archiveJobs.length == 0) {
                return;
            }

            var job = archiveJobs[0];

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
                                doArchiveJob();
                            });
                    }
                    else {
                        $scope.removeRow(job.row);
                        archiveJobs.shift();
                        if (archiveJobs.length == 0) {
                            console.log("No archive jobs left; closing modal.");
                            modal.close();
                        }
                        else {
                            doArchiveJob();
                        }
                    }
                });

        };

        doArchiveJob();

    };

    $scope.archiveByQuery = function () {
        if ($scope.response.hits.total == 0) {
            NotificationMessageService.add("warning", "No events to archive.");
            return;
        }

        var lastTimestamp = _.max($scope.aggregations, function (row) {
            return row.last_timestamp;
        }).last_timestamp;

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
            NotificationMessageService.add("warning", "No events to delete.");
            return;
        }

        var lastTimestamp = _.max($scope.aggregations, function (row) {
            return row.last_timestamp;
        }).last_timestamp;

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
                                range: {
                                    "@timestamp": {
                                        "lte": lastTimestamp
                                    }
                                }
                            }
                        ]
                    }
                }
            }
        };

        _.forEach($scope.filters, function (filter) {
            query.query.filtered.filter.and.push(filter);
        });

        ElasticSearch.deleteByQuery(query)
            .success(function (response) {
                NotificationMessageService.add("success", "Events deleted.");
                $scope.page = 1;
                $scope.refresh();
            })
            .error(function (error) {
                console.log(error);
            })
    };

    $scope.changeSortBy = function (what) {
        if ($scope.searchModel.sortBy == what) {
            $location.search("sortByOrder",
                    $scope.searchModel.sortByOrder == "desc" ? "asc" : "desc");
        }
        else {
            $location.search("sortBy", what);
        }
    };

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, "o", function () {
        $scope.$apply(function () {
            var row = $scope.getActiveRow();
            $scope.openAggregation(row);
        });
    });

    Keyboard.scopeBind($scope, "shift+j", function () {
        $scope.toggleActiveRowCheckbox();
        Mousetrap.trigger("j");
    });

    Keyboard.scopeBind($scope, "shift+k", function () {
        $scope.toggleActiveRowCheckbox();
        Mousetrap.trigger("k");
    });

    Keyboard.scopeBind($scope, "e", function () {
        $scope.$apply(function () {
            $scope.archiveSelected();
        });
    });

    Keyboard.scopeBind($scope, "#", function () {
        $scope.$apply(function () {
            $scope.deleteSelected();
        });
    });
});

