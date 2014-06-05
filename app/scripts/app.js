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

/*
 * TODO:
 * - Normalize on "hit", "result", "entry" to "event".
 * - Don't do DOM manipulation in controllers.
 */

var NAV_BAR_HEIGHT = 60;

var app = angular.module("app", [
    "ngRoute", "ngResource", "ui.bootstrap", "ui.bootstrap.pagination",
    "ui.bootstrap.modal"]);

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
        $location.url("/inbox");
        $scope.$apply();
    });

    Keyboard.scopeBind($scope, "g s", function (e) {
        $location.url("/starred");
        $scope.$apply();
    });

    Keyboard.scopeBind($scope, "g e", function (e) {
        $location.url("/events");
        $scope.$apply();
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

app.controller('AlertsController', function (Keyboard, $route, $location,
    $timeout, $routeParams, $scope, $http, $filter, Config, ElasticSearch, Util,
    $modal) {

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


    $scope.userQuery = $routeParams.q || "";
    if (_.isArray($scope.userQuery)) {
        $scope.userQuery = $scope.userQuery.join(" AND ");
    }

    $scope.page = $routeParams.page || 1;

    if ($routeParams.view == "inbox") {
        $scope.queryPrefix = "(event_type:alert AND tags:inbox)";
    }
    else if ($routeParams.view == "starred") {
        $scope.queryPrefix = "(event_type:alert AND tags:starred)";
    }
    else {
        $scope.queryPrefix = "(event_type:alert)";
    }

    $scope.buildQuery = function () {

        if ($scope.userQuery != "") {
            return $scope.queryPrefix + " AND (" + $scope.userQuery + ")";
        }
        else {
            return $scope.queryPrefix;
        }

        if ($scope.userQuery != "") {
            return "(event_type:alert AND tags:inbox) AND (" + $scope.userQuery + ")";
        }
        else {
            return "(event_type:alert AND tags:inbox)";
        }
    };

    $scope.displayErrorMessage = function (msg) {
        $scope.errorMessage = msg;
        $("#errorMessage").fadeIn();
        $("#errorMessage").delay(3000).fadeOut("slow");
    }

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
        $("#selectAllButton").blur();
    };

    $scope.deselectAll = function () {
        _.forEach($scope.hits.hits, function (hit) {
            hit.__selected = false;
        });
        $("#deselectAllButton").blur();
    };

    $scope.toggleOpenItem = function (item) {

        if (item.__open) {
            item.__open = false;
        }
        else {
            item.__open = true;

            // If open, do the scroll in a timeout as it has to be done after
            // apply.
            if (item.__open) {
                $timeout(function () {
                    $(window).scrollTop($("#" + item._id).offset().top - NAV_BAR_HEIGHT);
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

        var toArchive = _.filter($scope.hits.hits, function (hit) {
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
    $scope.severityToBootstrapClass = function (event) {
        switch (event._source.alert.severity) {
            case 1:
                return "danger";
                break;
            case 2:
                return "warning";
                break;
            default:
                return "info";
        }
    }

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
                                    query: $scope.buildQuery()
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

        if ($scope.aggregateBy == "signature") {
            request.size = 0;
            delete(request.from);
            request.aggs = {
                "signature": {
                    "terms": {
                        "field": "alert.signature.raw",
                        "order": {"_count": "desc"},
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

    $scope.handleSearchResponse = function (response) {
        $scope.response = response;
        $scope.hits = response.hits;
        $scope.activeRowIndex = 0;

        if ($scope.aggregateBy == "signature") {
            $scope.buckets = $scope.response.aggregations.signature.buckets;
        }
        else {
            $scope.buckets = undefined;
        }

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

        if ($scope.grouped != undefined) {
            $scope.rollUp();
        }
    };

    $scope.rollUp = function () {
        var grouped = _.groupBy($scope.hits.hits, function (event) {
            var key = event._source.alert.gid + ":" + event._source.alert.signature_id;
            return key;
        });
        $scope.grouped = _.sortBy(grouped, 'length').reverse();
        console.log(Util.formatString("Rolled up into {0} groups.", $scope.grouped.length));
    }

    $scope.renderIpAddress = function (addr) {
        addr = addr.replace(/0000/g, "");
        while (addr.indexOf(":0:") > -1) {
            addr = addr.replace(/:0:/g, "::");
        }
        addr = addr.replace(/:::+/g, "::");
        return addr;
    };

    $scope.archiveByQuery = function () {
        if ($scope.hits == undefined || $scope.hits.hits.length == 0) {
            $scope.displayErrorMessage("No events to archive.");
            return;
        }

        var lastTimestamp = $scope.hits.hits[0]._source["@timestamp"];
        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.buildQuery()
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

        $modal.open({
            templateUrl: "templates/archive-events-by-query-modal.html",
            controller: "ArchiveEventsByQueryModal",
            resolve: {
                args: function () {
                    return {
                        "title": "Archiving...",
                        "query": query
                    }
                }
            }
        }).result.then(function () {
                $scope.page = 1;
                $scope.refresh();
            });

    };

    $scope.deleteByQuery = function () {
        if ($scope.hits == undefined || $scope.hits.hits.length == 0) {
            $scope.displayErrorMessage("No events to delete.");
            return;
        }

        var latestTimestamp = $scope.hits.hits[0]._source["@timestamp"];

        var query = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.buildQuery()
                        }
                    },
                    filter: {
                        and: [
                            {
                                range: {
                                    "@timestamp": {
                                        "lte": latestTimestamp
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
    }

    var toggleSelected = function () {
        var event = $scope.hits.hits[$scope.activeRowIndex];
        event.__selected = !event.__selected;
    };

    $scope.increaseRequestSize = function () {
        $scope.querySize = $scope.querySize * 2;
        $scope.refresh();
    };

    $scope.decreaseRequestSize = function () {
        $scope.querySize = $scope.querySize / 2;
        $scope.refresh();
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

    Keyboard.scopeBind($scope, "^", function() {
        $("#aggregate-by-input").focus();
    });

    Keyboard.scopeBind($scope, "j", function (e) {
        $scope.$apply(function () {
            console.log("AlertsController: keyboard: j");
            moveToNextEntry();
        });
    });

    Keyboard.scopeBind($scope, "shift+j", function (e) {
        $scope.$apply(function () {
            toggleSelected();
            moveToNextEntry();
        });
    })

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
            ;
        });
    });

    Keyboard.scopeBind($scope, "* a", function (e) {
        $scope.$apply($scope.selectAll());
    });

    Keyboard.scopeBind($scope, "* n", function (e) {
        $scope.$apply($scope.deselectAll());
    });

    Keyboard.scopeBind($scope, "o", function (e) {
        $scope.$apply(function () {
            $scope.toggleOpenItem($scope.hits.hits[$scope.activeRowIndex]);
        });
    });

    Keyboard.scopeBind($scope, "e", function (e) {
        $scope.$apply($scope.archiveSelected());
    });

    Keyboard.scopeBind($scope, "#", function (e) {
        $scope.$apply($scope.deleteSelected());
    });

    Keyboard.scopeBind($scope, ">", function (e) {
        if ($scope.page * Config.elasticSearch.size < $scope.hits.total) {
            $scope.page++;
            $scope.refresh();
        }
    });

    Keyboard.scopeBind($scope, "<", function (e) {
        if ($scope.page > 1) {
            $scope.page--;
            $scope.refresh();
        }
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
});
