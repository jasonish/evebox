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

app.controller("NavBarController", function ($routeParams, $scope, $modal,
    $location, Keyboard, EventRepository, $timeout) {

    $scope.$routeParams = $routeParams;

    $scope.openConfig = function () {
        $modal.open({
            templateUrl: "templates/config.html",
            controller: "ConfigController as vm"
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

app.controller("RecordController", function ($scope, $routeParams, Util,
    ElasticSearch, Config, Cache) {

    // Export some functions to $scope.
    $scope.Util = Util;
    $scope.Config = Config;

    var eventId = $routeParams.id;
    var eventCache = Cache.get("events");
    if (eventId in eventCache) {
        console.log("Found event in cache.");
    }
    else {
        console.log("Event not found in cache.");
    }

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
    try {
        $scope.payloadIsBase64 = Util.isBase64($scope.hit._source.payload);
        $scope.hasPayload = true;
    }
    catch (err) {
        $scope.payloadIsBase64 = false;
        $scope.hasPayload = false;
    }

    $scope.b64ToText = function (data) {
        return atob(data);
    };

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

    $scope.sendToDumpy = function (event) {
        var form = document.createElement("form");
        form.setAttribute("method", "post");
        form.setAttribute("action", Config.dumpy.url);
        form.setAttribute("target", "_blank");

        var eventInput = document.createElement("input");
        eventInput.setAttribute("type", "hidden");
        eventInput.setAttribute("name", "event");
        eventInput.setAttribute("value", angular.toJson(event._source));
        form.appendChild(eventInput);

        form.submit();
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

    $scope.$routeParams = $routeParams;
    $scope.Util = Util;
    $scope.querySize = Config.elasticSearch.size;

    $scope.searchModel = {
        userQuery: $routeParams.q || "",
        page: $routeParams.page || 1
    };

    $scope.filters = [
        /* Limit the result set to documents with an event_type field. */
        {
            "exists": {"field": "event_type"}
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
            from: Config.elasticSearch.size * (($scope.searchModel.page || 1) - 1),
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

