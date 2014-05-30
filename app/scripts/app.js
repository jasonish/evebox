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
        controller: "MainController",
        templateUrl: "views/events.html"
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

app.controller('MainController', function (Keyboard, $route, $location,
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
    $scope.loading = false;
    $scope.state = "";
    $scope.errorMessage = "";
    $scope.page = 1;
    $scope.userQuery = "";
    $scope.currentSelectionIdx = 0;
    $scope.toJson = Util.toJson;

    if ($routeParams.view == "inbox") {
        $scope.queryPrefix = "(event_type:alert AND tags:inbox)";
    }
    else if ($routeParams.view == "starred") {
        $scope.queryPrefix = "(event_type:alert AND tags:starred)";
    }
    else {
        $scope.queryPrefix = "(event_type:alert)";
    }

    if ("q" in $routeParams) {
        $scope.userQuery = $routeParams.q;
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
        _.remove($scope.hits.hits, hit);

        // Update the currently selected item.
        var newIdx = $scope.hits.hits.indexOf($scope.currentSelection);
        if (newIdx >= 0) {
            $scope.currentSelectionIdx = newIdx;
        }
        else {
            if ($scope.currentSelectionIdx >= $scope.hits.hits.length) {
                $scope.currentSelectionIdx = $scope.hits.hits.length - 1;
            }
            $scope.currentSelection = $scope.hits.hits[$scope.currentSelectionIdx];
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
    }

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

    var scrollIdIntoView = function (elementId) {
        var element = $("#" + elementId);
        if (!isScrolledIntoView(element)) {
            $(window).scrollTop(element.position().top - ($(window).height() / 2));
        }
    };

    var toggleCurrentSelection = function () {
        $scope.currentSelection.__selected = !$scope.currentSelection.__selected;
    };

    var setActiveAvent = function (event) {
        if (_.isNumber(event)) {
            $scope.currentSelectionIdx = event;
            $scope.currentSelection = $scope.hits.hits[$scope.currentSelectionIdx];
        }
    };

    var moveToNextEntry = function () {
        if ($scope.currentSelectionIdx + 1 < $scope.hits.hits.length) {
            $scope.currentSelectionIdx += 1;
            $scope.currentSelection = $scope.hits.hits[$scope.currentSelectionIdx];
            scrollIdIntoView($scope.currentSelection._id);
        }
    };

    var moveToPreviousEntry = function () {
        if ($scope.currentSelectionIdx > 0) {
            $scope.currentSelectionIdx -= 1;
            $scope.currentSelection = $scope.hits.hits[$scope.currentSelectionIdx];
            if ($scope.currentSelectionIdx == 0) {
                $(window).scrollTop(0);
            }
            else {
                scrollIdIntoView($scope.currentSelection._id);
            }
        }
    };

    var handleSearchResponse = function (response) {
        $scope.hits = response.hits;
        $scope.currentSelection = $scope.hits.hits[0];
        $scope.currentSelectionIdx = 0;

        _.forEach($scope.hits.hits, function (hit) {
            hit._source["@timestamp"] =
                moment(hit._source["@timestamp"]).format();

            // Add a tags list if it doesn't exist.
            if (hit._source.tags == undefined) {
                hit._source.tags = [];
            }

        });
    };

    $scope.refresh = function () {
        $("#query-input").blur();
        var request = {
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: $scope.buildQuery()
                        }
                    }
                }
            },
            size: Config.elasticSearch.size,
            from: Config.elasticSearch.size * ($scope.page - 1),
            sort: [
                {"@timestamp": {order: "desc"}}
            ]
        };

        $scope.queryAsString = angular.toJson(request);
        $scope.submitSearch(request);
    };

    $scope.submitSearch = function (request) {
        $scope.loading = true;
        ElasticSearch.search(request).success(function (response) {
            handleSearchResponse(response);
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
        var modal = $modal.open({
            templateUrl: "templates/archive-by-query-progress-modal.html",
            controller: "ArchiveByQueryProgressModal",
            resolve: {
                inboxScope: function () {
                    return $scope;
                }
            }
        }).result.then(function () {
                console.log("modal is closed");
                $scope.refresh();
            });
    };

    $scope.deleteByQuery = function() {
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
                $scope.refresh();
            })
            .error(function (error) {
                console.log(error);
            })
    }

    $scope.refresh();

    /*
     * Keyboard bindings.
     */

    $scope.$on("$destroy", function () {
        Keyboard.resetScope($scope);
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
    })

    Keyboard.scopeBind($scope, "j", function (e) {
        $scope.$apply(function () {
            moveToNextEntry();
        });
    });

    Keyboard.scopeBind($scope, "shift+j", function (e) {
        $scope.$apply(function () {
            toggleCurrentSelection();
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
            toggleCurrentSelection();
            moveToPreviousEntry();
        });
    });

    Keyboard.scopeBind($scope, "x", function (e) {
        $scope.currentSelection.__selected = !$scope.currentSelection.__selected;
        $scope.$apply();
    });

    Keyboard.scopeBind($scope, "s", function (e) {
        $scope.toggleStar($scope.currentSelection);
        $scope.$apply();
    });

    Keyboard.scopeBind($scope, "* a", function (e) {
        $scope.$apply($scope.selectAll());
    });

    Keyboard.scopeBind($scope, "* n", function (e) {
        $scope.$apply($scope.deselectAll());
    });

    Keyboard.scopeBind($scope, "o", function (e) {
        $scope.toggleOpenItem($scope.currentSelection);
        $scope.$apply();
    });

    Keyboard.scopeBind($scope, "e", function (e) {
        $scope.$apply($scope.archiveSelected());
    });

    Keyboard.scopeBind($scope, "#", function (e) {
        $scope.$apply($scope.deleteSelected());
    });

    Keyboard.scopeBind($scope, ">", function (e) {
        $scope.page++;
        $scope.refresh();
    });

    Keyboard.scopeBind($scope, "<", function (e) {
        if ($scope.page > 0) {
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
        $(".dropdown-toggle").first().dropdown("toggle");
    });

})
;

/*
 * Non-Angular code - utility functions, etc.
 */

/**
 * Check if an element is currently within the visible area of the window.
 *
 * From http://stackoverflow.com/questions/487073/check-if-element-is-visible-after-scrolling.
 */
function isScrolledIntoView(elem) {
    var docViewTop = $(window).scrollTop() + NAV_BAR_HEIGHT;
    var docViewBottom = docViewTop + $(window).height() - NAV_BAR_HEIGHT;

    var elemTop = $(elem).offset().top;
    var elemBottom = elemTop + $(elem).height();

    return ((elemBottom <= docViewBottom) && (elemTop >= docViewTop));
}
