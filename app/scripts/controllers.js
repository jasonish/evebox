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

app.controller("ArchiveEventsByQueryModal", function ($scope, ElasticSearch,
    args) {

    $scope.title = args.title;
    $scope.archived = 0;
    $scope.total = undefined;

    modalScope = $scope;

    var searchThenArchive = function () {
        ElasticSearch.search(args.query)
            .success(function (response) {
                if ($scope.total == undefined) {
                    $scope.total = response.hits.total;
                }
                if (response.hits.hits.length > 0) {
                    ElasticSearch.bulkRemoveTag(response.hits.hits, "inbox")
                        .success(function (response) {
                            $scope.archived += response.items.length;
                            searchThenArchive();
                        });
                }
                else {
                    $scope.$close();
                }
            });
    };

    searchThenArchive();

});

app.controller("AggregatedController", function ($scope, $location, Keyboard,
    ElasticSearch, $modal, $routeParams) {

    AggregatedController = $scope;

    var getActiveBucket = function () {
        return $scope.buckets[$scope.activeRowIndex];
    };

    /**
     * Opens an aggregation in "flat" view by explicitly setting no aggregation
     * and constructing a query.
     */
    $scope.openAggregation = function (bucket) {
        var searchParams = $location.search();
        searchParams.aggregateBy = "";
        searchParams.q = "q" in searchParams ? searchParams.q : "";
        searchParams.q += " +alert.signature.raw:\"" + bucket.key + "\"";
        searchParams.q = searchParams.q.trim();

        $location.search(searchParams);
    };

    $scope.toggleActiveRow = function () {
        var bucket = getActiveBucket();
        bucket.__selected = !bucket.__selected;
    };

    $scope.selectAll = function () {
        _.forEach($scope.buckets, function (bucket) {
            bucket.__selected = true;
        });
    };

    $scope.deselectAll = function () {
        _.forEach($scope.buckets, function (bucket) {
            bucket.__selected = false;
        });
    };

    $scope.selectedCount = function () {
        try {
            return _.filter($scope.buckets, function (bucket) {
                return bucket.__selected;
            }).length;
        }
        catch (err) {
            return 0;
        }
    };

    $scope.totalEventCount = function () {
        return _.reduce($scope.buckets, function (sum, bucket) {
            return sum + bucket.doc_count;
        }, 0);
    };

    $scope.deleteBucket = function (bucket) {
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
                    "lte": bucket.last_timestamp.value
                }
            }
        });

        query.query.filtered.filter.and.push({
            "term": {
                "alert.signature.raw": bucket.key
            }
        });

        return ElasticSearch.deleteByQuery(query);

    };

    $scope.deleteSelected = function () {
        var selectedBuckets = _.filter($scope.buckets, "__selected");

        if (!selectedBuckets) {
            return;
        }

        var bucket = selectedBuckets.pop();

        $scope.deleteBucket(bucket)
            .success(function (response) {
                _.remove($scope.buckets, bucket);
                if ($scope.activeRowIndex > 0 && _.indexOf($scope.buckets, bucket) < $scope.activeRowIndex) {
                    $scope.activeRowIndex--;
                }
            })
            .finally(function () {
                $scope.deleteSelected();
            });
    };

    $scope.archiveBucket = function (bucket) {
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
                                        "lte": bucket.last_timestamp.value
                                    }
                                }
                            },
                            {
                                term: {
                                    "alert.signature.raw": bucket.key
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

        return $modal.open({
            templateUrl: "templates/archive-events-by-query-modal.html",
            controller: "ArchiveEventsByQueryModal",
            resolve: {
                args: function () {
                    return {
                        "title": "Archiving: " + bucket.key,
                        "query": query
                    }
                }
            }
        });
    };

    $scope.archiveSelected = function () {
        var selectedBuckets = _.filter($scope.buckets, "__selected");

        if (selectedBuckets.length == 0) {
            return $scope.displayErrorMessage("No events selected.");
        }

        var archiveBucket = function () {
            if (selectedBuckets.length > 0) {
                var bucket = selectedBuckets.pop();
                $scope.archiveBucket(bucket)
                    .result.then(function () {
                        var bucketIndex = _.indexOf($scope.buckets, bucket);
                        if (($scope.activeRowIndex > 0) && (bucketIndex <= $scope.activeRowIndex)) {
                            $scope.activeRowIndex--;
                        }
                        _.remove($scope.buckets, bucket);
                        archiveBucket();
                    });
            }
        };

        archiveBucket();
    };

    $scope.archiveByQuery = function () {
        if ($scope.response.hits.total == 0) {
            $scope.displayErrorMessage("No events to archive.");
            return;
        }

        var lastTimestamp = _.max($scope.buckets, function (bucket) {
            return bucket.last_timestamp.value;
        }).last_timestamp.value;

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

        var lastTimestamp = _.max($scope.buckets, function (bucket) {
            return bucket.last_timestamp.value;
        }).last_timestamp.value;

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
        $location.search("sortBy", what);
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
            var bucket = getActiveBucket();
            $scope.openAggregation(bucket);
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
        console.log("AggregatedController: e");
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