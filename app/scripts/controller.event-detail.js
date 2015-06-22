'use strict';

(function() {

    angular.module("app").controller("EventDetailController",
        EventDetailController);

    function EventDetailController($scope, Mousetrap, Config,
                                   ElasticSearch, EventRepository, Util) {

        console.log("EventDetailController");

        var vm = this;

        vm.event = $scope.event;

        $scope.Config = Config;
        $scope.Util = Util;
        $scope._ = _;

        /* Suricata can store the payload as base64 or printable.  Attempt to
         * guess which it is here. */
        try {
            $scope.payloadIsBase64 = Util.isBase64(vm.event._source.payload);
            $scope.hasPayload = true;
        }
        catch (err) {
            $scope.payloadIsBase64 = false;
            $scope.hasPayload = false;
        }

        $scope.b64ToText = function(data) {
            return atob(data);
        };

        $scope.b64ToHex = function(data) {
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

        vm.buildSearchByFlowUrl = function(hit) {

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

        $scope.archiveEvent = function(event) {
            if ($scope.$parent.archiveEvent === undefined) {
                ElasticSearch.removeTag(event, "inbox")
                    .success(function(response) {
                        _.remove(event._source.tags, function(tag) {
                            return tag == "inbox";
                        })
                    });
            }
            else {
                $scope.$parent.archiveEvent(event);
            }
        };

        $scope.deleteEvent = function(event) {
            if ($scope.$parent.deleteEvent === undefined) {
                EventRepository.deleteEvent(event)
                    .success(function(response) {
                        $scope.$emit("eventDeleted", event);
                    });
            }
            else {
                $scope.$parent.deleteEvent(event);
            }
        };

        $scope.toggleStar = function(event) {
            EventRepository.toggleStar(event);
        };

        $scope.sendToDumpy = function(event) {
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

        EventRepository.lookupRrname(vm.event._source.dest_ip,
            vm.event._source["@timestamp"])
            .then(function(rrname) {
                vm.destHostname = rrname || null;
            });

        EventRepository.lookupRrname(vm.event._source.src_ip,
            vm.event._source["@timestamp"])
            .then(function(rrname) {
                vm.srcHostname = rrname || null;
            });

        Mousetrap.bind($scope, ".", function() {
            $("#event-detail-more-button").dropdown('toggle');
        }, "Open More Menu");

    };

})();