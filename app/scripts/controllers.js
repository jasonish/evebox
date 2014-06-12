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
    $location, Keyboard) {

    $scope.$routeParams = $routeParams;

    $scope.openConfig = function () {
        $modal.open({
            templateUrl: "views/config.html",
            controller: "ConfigController"
        });
    };

    $scope.openHelp = function () {
        $modal.open({
            templateUrl: "views/help.html",
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

    Keyboard.scopeBind($scope, "g e", function (e) {
        $scope.$apply(function () {
            $location.url("/events");
        });
    });

    Keyboard.scopeBind($scope, "g c", function (e) {
        $scope.openConfig();
    });

    Keyboard.scopeBind($scope, "?", function (e) {
        $scope.openHelp();
    })
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
    ElasticSearch) {

    // Export some functions to $scope.
    $scope.Util = Util;

    ElasticSearch.searchEventById($routeParams.id)
        .success(function (response) {
            $scope.response = response;
            $scope.hits = response.hits;
        });

});

app.controller("EventDetailController", function ($scope, Keyboard) {

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
    });

    Keyboard.scopeBind($scope, ".", function () {
        $("#event-detail-more-button").first().dropdown('toggle');
    });

});

app.controller("ModalProgressController", function ($scope, jobs) {

    $scope.jobs = jobs;

});

app.controller("AggregatedController", function ($scope, $location, Keyboard,
    ElasticSearch, $modal, $routeParams) {

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
                            query: $scope.userQuery || "*"
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
            fields: ["_index", "_type", "_id"],
            sort: [
                {"@timestamp": {order: "desc"}}
            ]
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
            return $scope.displayErrorMessage("No events selected.");
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
                        var row = job.row;
                        var rowIndex = _.indexOf($scope.aggregations, row);
                        if (($scope.activeRowIndex > 0) && (rowIndex <= $scope.activeRowIndex)) {
                            $scope.activeRowIndex--;
                        }
                        _.remove($scope.aggregations, row);
                        _.remove(archiveJobs, job);
                        if (archiveJobs.length == 0) {
                            console.log("No archive jobs left; closing modal.");
                            modal.close();
                        }
                    }
                });

        };

        _.forEach(archiveJobs, doArchiveJob);

    };

    $scope.archiveByQuery = function () {
        if ($scope.response.hits.total == 0) {
            $scope.displayErrorMessage("No events to archive.");
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

        var lastTimestamp = _.max($scope.aggregations, function (row) {
            return row.last_timestamp;
        }).last_timestamp;

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
        if ($scope.sortBy == what) {
            $location.search("sortByOrder",
                $scope.sortByOrder == "desc" ? "asc" : "desc");
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