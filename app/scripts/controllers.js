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
                else {
                    hit.__title = hit._source.event_type.toUpperCase();
                    hit.__titleClass = "alert-info";
                }

            });

        });

});

app.controller("EventDetailController", function ($scope, Keyboard, Config,
    ElasticSearch, EventRepository) {

    console.log("EventDetailController");

    $scope.Config = Config;

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

app.controller("AggregatedController", function ($scope, $location, Keyboard,
    ElasticSearch, $modal, $routeParams, NotificationMessageService) {

    console.log("AggregatedController");

    AggregatedController = $scope;

    $scope.activeRowIndex = 0;

    var getActiveRow = function () {
        return $scope.aggregations[$scope.activeRowIndex];
    };

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

    $scope.toggleActiveRow = function () {
        var row = getActiveRow();
        row.__selected = !row.__selected;
    };

    $scope.selectAll = function () {
        _.forEach($scope.aggregations, function (agg) {
            agg.__selected = true;
        });
    };

    $scope.deselectAll = function () {
        _.forEach($scope.aggregations, function (agg) {
            agg.__selected = false;
        });
    };

    $scope.selectedCount = function () {
        try {
            return _.filter($scope.aggregations, function (agg) {
                return agg.__selected;
            }).length;
        }
        catch (err) {
            return 0;
        }
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
                            query: $scope.searchForm.userQuery || "*"
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
        var selectedRows = _.filter($scope.aggregations, "__selected");

        if (!selectedRows) {
            return;
        }

        var row = selectedRows.pop();

        $scope.deleteRow(row)
            .success(function (response) {
                _.remove($scope.aggregations, row);
                if ($scope.activeRowIndex > 0 && _.indexOf($scope.aggregations, row) < $scope.activeRowIndex) {
                    $scope.activeRowIndex--;
                }
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
                            query: $scope.searchForm.userQuery || "*"
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
        var selectedRows = _.filter($scope.aggregations, "__selected");

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
                        var row = job.row;
                        var rowIndex = _.indexOf($scope.aggregations, row);
                        if (($scope.activeRowIndex > 0) && (rowIndex <= $scope.activeRowIndex)) {
                            $scope.activeRowIndex--;
                        }
                        _.remove($scope.aggregations, row);
                        //_.remove(archiveJobs, job);
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
                            query: $scope.searchForm.userQuery || "*"
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
                            query: $scope.searchForm.userQuery || "*"
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

        if ($routeParams.view == "inbox") {
            query.query.filtered.filter.and.push({term: {tags: "inbox"}});
        }

        ElasticSearch.deleteByQuery(query)
            .success(function (response) {
                $scope.page = 1;
                $scope.refresh();
            })
            .error(function (error) {
                console.log(error);
            })
    };

    $scope.changeSortBy = function (what) {
        if ($scope.searchForm.sortBy == what) {
            $location.search("sortByOrder",
                    $scope.searchForm.sortByOrder == "desc" ? "asc" : "desc");
        }
        else {
            $location.search("sortBy", what);
        }
    };

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, "* a", function (e) {
        $scope.$apply($scope.selectAll());
    });

    Keyboard.scopeBind($scope, "* n", function (e) {
        $scope.$apply($scope.deselectAll());
    });

    Keyboard.scopeBind($scope, "o", function () {
        $scope.$apply(function () {
            var row = getActiveRow();
            $scope.openAggregation(row);
        });
    });

    Keyboard.scopeBind($scope, "x", function () {
        $scope.$apply(function () {
            $scope.toggleActiveRow();
        });
    });

    Keyboard.scopeBind($scope, "shift+j", function () {
        $scope.toggleActiveRow();
        Mousetrap.trigger("j");
    });

    Keyboard.scopeBind($scope, "shift+k", function () {
        $scope.toggleActiveRow();
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

app.controller("EventsController", function ($scope, Util, Keyboard, Config,
    ElasticSearch, $routeParams, $location) {

    console.log("EventsController");

    EventsController = $scope;

    $scope.Util = Util;

    $scope.page = $routeParams.page || 1;
    $scope.querySize = Config.elasticSearch.size;

    $scope.searchForm = {
        userQuery: $routeParams.q || ""
    };

    $scope.filters = [
        {
            "exists": { "field": "event_type" }
        }
    ];

    $scope.activeRowIndex = 0;

    $scope.eventMessage = function (event) {
        switch (event.event_type) {
            default:
                return angular.toJson(event[event.event_type]);
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

        console.log(response);

        $scope.rows = response.hits.hits.map(function (hit) {

            // If row is an event, set the class based on the severity.
            if (hit._source.alert) {
                var trClass = Util.severityToBootstrapClass(
                    hit._source.alert.severity);
            }
            else {
                var trClass = "info";
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

        $scope.rows = _.sortBy($scope.rows, function (row) {
            return Util.timestampToFloat(row.source._source.timestamp);
        }).reverse();

        $scope.columnStyles = {
            "timestamp": {"white-space": "nowrap"},
            "message": {"word-break": "break-all"}
        };
    };

    $scope.toggleOpenEvent = function (event) {
        _.forEach($scope.rows, function (row) {
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

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, "o", function () {
        $scope.$apply(function () {
            console.log($scope.activeRowIndex);
            $scope.toggleOpenEvent($scope.rows[$scope.activeRowIndex]);
        });
    });

    $scope.doSearch = function () {

        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.searchForm.userQuery || "*"
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
        $location.search("q", $scope.searchForm.userQuery);
    };

    $scope.$on("eventDeleted", function (e, event) {
        _.remove($scope.rows, function (row) {
            return row.source === event;
        });
    });

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    $scope.doSearch();
});
