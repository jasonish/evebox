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

(function () {

    'use strict';

    app.controller("NavBarController", function ($routeParams, $scope, $modal,
        $location, EventRepository, $timeout, Mousetrap) {

        $scope.$routeParams = $routeParams;
        $scope.$location = $location;

        $scope.openConfig = function () {
            $modal.open({
                templateUrl: "templates/config.html",
                controller: "ConfigController as vm"
            });
        };

        $scope.openHelp = function () {
            $modal.open({
                templateUrl: "templates/help.html",
                controller: "HelpController as vm",
                size: "lg"
            });
        };

        Mousetrap.bind($scope, "g i", function (e) {
            $location.url("/inbox");
        }, "Go to Inbox");

        Mousetrap.bind($scope, "g s", function (e) {
            $location.url("/starred")
        }, "Go to Starred");

        Mousetrap.bind($scope, "g a", function (e) {
            $location.url("/alerts");
        }, "Go to Alerts");

        Mousetrap.bind($scope, "g o", function () {
            $("#other-menu-dropdown-toggle").dropdown('toggle');
        }, "Go to Other");

        Mousetrap.bind($scope, "g c", function (e) {
            $scope.openConfig();
        }, "Go to Configuration");

        Mousetrap.bind($scope, "?", function (e) {
            $scope.openHelp();
        }, "Show Help");
    });

    app.controller("EventDetailController", function ($scope, Mousetrap, Config,
        ElasticSearch, EventRepository, Util) {

        var vm = this;

        console.log("EventDetailController");

        if ($scope.event) {
            $scope.hit = $scope.event;
        }

        $scope.Config = Config;
        $scope.Util = Util;
        $scope._ = _;

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
            for (var i = 0; i < hex.length; i ++) {
                if (i > 0 && i % 16 == 0) {
                    buf += "\n";
                }
                buf += hex[i] + " ";
            }
            return buf;
        };

        vm.buildSearchByFlowUrl = function (hit) {

            var query = Util.printf('flow_id:{}' +
                ' src_ip.raw:("{}" OR "{}")' +
                ' dest_ip.raw:("{}" OR "{}")',
                hit._source.flow_id,
                hit._source.src_ip,
                hit._source.dest_ip,
                hit._source.src_ip,
                hit._source.dest_ip);

            if (hit._source.src_port && hit._source.dest_port) {
                query += Util.printf(' src_port:({} OR {})' +
                    ' dest_port:({} OR {})',
                    hit._source.src_port,
                    hit._source.dest_port,
                    hit._source.src_port,
                    hit._source.dest_port);
            }
            else {
                query += Util.printf(' proto:{}',
                    hit._source.proto);
            }

            return encodeURIComponent(query);
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

        var lookupRrname = function (addr, lteTimestamp) {
            return EventRepository.queryEvents({
                filters: [
                    {term: {event_type: "dns"}},
                    {term: {"dns.rdata.raw": addr}},
                    {range: {"@timestamp": {lte: lteTimestamp}}}
                ],
                size: 1,
                sort: [
                    {"@timestamp": {order: "desc"}}
                ]
            }).then(function (result) {
                if (result.data.hits.hits.length > 0) {
                    return result.data.hits.hits[0]._source.dns.rrname;
                }
            });
        };

        lookupRrname($scope.hit._source.dest_ip,
            $scope.hit._source["@timestamp"])
            .then(function (rrname) {
                vm.destinationHostname = rrname;
            });

        lookupRrname($scope.hit._source.src_ip,
            $scope.hit._source["@timestamp"])
            .then(function (rrname) {
                vm.sourceHostname = rrname;
            });

        Mousetrap.bind($scope, ".", function () {
            $("#event-detail-more-button").dropdown('toggle');
        }, "Open More Menu");

    });

    angular.module("app").controller("HelpController", HelpController);

    function HelpController(Mousetrap) {
        var vm = this;
        vm.bindings = Mousetrap.bindings;
        vm.sortedKeys = _.sortBy(_.keys(Mousetrap.bindings));
    };

})();
