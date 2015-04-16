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

(function() {

    angular.module("app").controller("AlertsController",
        AlertsController);

    function AlertsController($scope, EventRepository, $routeParams,
                              $location, baseUrl, Mousetrap, Util, $timeout, NotificationService) {

        var vm = this;
        vm.$routeParams = $routeParams;
        vm.activeRowIndex = 0;
        vm.openEventIndex = -1;
        vm.baseUrl = baseUrl;
        if (!$routeParams.page) {
            $routeParams.page = 1;
        }
        else {
            $routeParams.page = parseInt($routeParams.page);
        }

        /* Set to true when loading/refreshing.  False otherwise.
         * Used to update the UI when loading. */
        vm.loading = true;

        var sortByTimestamp = function(desc) {
            vm.events = _.sortBy(vm.events, function(event) {
                return event.timestamp;
            });
            if (desc) {
                vm.events.reverse();
            }
        };

        vm.selectAll = function() {
            _.forEach(vm.events, function(event) {
                event.selected = true;
            });
        };

        vm.deselectAll = function() {
            _.forEach(vm.events, function(event) {
                event.selected = false;
            });
        };

        var selectBySeverity = function(severity) {
            _.forEach(vm.events, function(event) {
                if (event._source.alert.severity == severity) {
                    event.selected = true;
                }
            })
        };
        vm.selectBySeverity = selectBySeverity;

        vm.gotoView = function(view) {
            $location.path(baseUrl + "/" + view);
        };

        vm.gotoPage = function(page) {
            $location.search({page: page});
        };

        vm.openEvent = function(event) {
            var index = _.indexOf(vm.events, event);
            if (vm.openEventIndex == index) {
                vm.openEventIndex = -1;
            }
            else {
                vm.openEventIndex = index;
                vm.activeRowIndex = index;

                /* XXX Scrolls the open event to the top. */
                $timeout(function() {
                    var element = $("table[event-table] tbody")
                        .eq(vm.openEventIndex);
                    $("html, body").animate(
                        {scrollTop: element.offset().top}, 100);

                });
            }
        };

        vm.openGroup = function(event) {
            var query = Util.printf(
                '{} +alert.signature.raw:"{}"',
                ($location.search().q || ""),
                event._source.alert.signature);
            $location.path(baseUrl + "/flat");
            $location.search({q: query.trim()});
        };

        vm.open = function(event) {
            if (event.count === undefined) {
                vm.openEvent(event);
            }
            else {
                vm.openGroup(event);
            }
        };

        vm.submitSearchForm = function() {
            $location.search({q: $routeParams.q});
        };

        vm.refresh = function() {
            load();
        };

        /**
         * Toggle the selected state of an event.
         */
        var toggleSelect = function(event) {
            if (event === undefined) {
                event = vm.events[vm.activeRowIndex];
            }
            event.selected = !event.selected;
        };

        /**
         * Remove an event from the event list, updating the activeRowIndex
         * if required.
         */
        var removeEvent = function(event) {
            var index = _.indexOf(vm.events, event);
            if (index == vm.openEventIndex) {
                vm.openEventIndex = -1;
            }
            if (vm.activeRowIndex > index) {
                vm.activeRowIndex--;
            }
            else if (vm.activeRowIndex == vm.events.length - 1) {
                vm.activeRowIndex--;
            }
            _.remove(vm.events, event);

            // Update the total event count in the result object.
            vm.result.total -= event.count || 1;

            if (vm.events.length == 0) {
                vm.refresh();
            }
        };

        var archiveGroup = function(selected) {
            if (selected.length == 0) {
                return;
            }

            _.forEach(selected, function(group) {
                group.archiving = true;
            });

            var group = selected.pop();
            var filters = _.cloneDeep(vm.filters);
            for (var key in group.keys) {
                var filter = {term: {}};
                filter.term[key] = group.keys[key];
                filters.push(filter);
            }

            EventRepository.archiveByQuery({
                query: $routeParams.q,
                filters: filters,
                lteTimestamp: group.timestamp
            }).then(function() {
                removeEvent(group);
                archiveGroup(selected);
            }, function(error) {
                NotificationService.add("danger", error);
            });
        };

        /**
         * Recursively delete each event group, use selected as a stack as too
         * many simultaneous delete requests can result in ES returning an
         * error.
         */
        var deleteGroup = function(selected) {
            if (selected.length == 0) {
                return;
            }

            var group = selected.pop();
            var filters = _.cloneDeep(vm.filters);
            for (var key in group.keys) {
                var filter = {term: {}};
                filter.term[key] = group.keys[key];
                filters.push(filter);
            }

            EventRepository.deleteByQuery({
                query: $routeParams.q,
                filters: filters,
                lteTimestamp: group.timestamp
            }).then(function() {
                removeEvent(group);
                deleteGroup(selected);
            })
        };

        vm.getSelected = function() {
            return _.filter(vm.events, "selected");
        };

        vm.archiveSelected = function() {
            var selected = vm.getSelected();
            if (selected.length == 0) {
                return;
            }
            if (selected[0].count) {
                archiveGroup(selected);
            }
            else {
                _.forEach(selected, function(event) {
                    event.archiving = true;
                    EventRepository.archiveEvent(event)
                        .then(
                        function(result) {
                            removeEvent(event);
                        });
                });
            }
        };

        vm.deleteSelected = function() {
            var selected = vm.getSelected();
            if (selected.length == 0) {
                return;
            }

            _.forEach(selected, function(event) {
                event.deleting = true;
            });

            if (selected[0].count) {
                deleteGroup(selected);
            }
            else {
                _.forEach(selected, function(event) {
                    EventRepository.deleteEvent(event).then(
                        function(result) {
                            removeEvent(event);
                        }
                    );
                });
            }
        };

        var load = function() {

            vm.loading = true;
            vm.activeRowIndex = 0;

            if ($routeParams.view == "flat") {
                EventRepository.getEvents({
                    filters: vm.filters,
                    query: $routeParams.q,
                    page: $routeParams.page
                }).then(
                    function(result) {
                        vm.result = result;
                        vm.events = result.events;
                        vm.loading = false;
                    },
                    function(result) {
                        NotificationService.add("danger",
                            Util.printf("Error: {}",
                                result.statusText));
                        console.log("error:");
                        console.log(result);
                    }
                );
            }
            else if ($routeParams.view == "signature") {
                EventRepository.getAlertsGroupedBySignature({
                    filters: vm.filters,
                    query: $routeParams.q
                }).then(
                    function(result) {
                        vm.result = result;
                        vm.events = result.events;
                        sortByTimestamp(true);
                        vm.loading = false;
                    },
                    function(result) {
                        NotificationService.add("danger",
                            Util.printf("Error: {}",
                                result.statusText));
                        console.log("error:");
                        console.log(result);
                    }
                );
            }
            else if ($routeParams.view == "signature+src") {
                EventRepository.getAlertsGroupedBySignatureAndSource({
                    filters: vm.filters,
                    query: $routeParams.q
                }).then(
                    function(result) {
                        vm.result = result;
                        vm.events = result.events;
                        sortByTimestamp(true);
                        vm.loading = false;
                    },
                    function(result) {
                        NotificationService.add("danger",
                            Util.printf("Error: {}",
                                result.statusText));
                        console.log("error:");
                        console.log(result);
                    }
                );
            }

        };

        var getActiveEvent = function() {
            return vm.events[vm.activeRowIndex];
        };

        vm.toggleStar = function(event) {
            if (!event.count) {
                EventRepository.toggleStar(event);
            }
        };

        // Init.
        (function() {

            Mousetrap.bind($scope, "r", vm.refresh, "Refresh");
            Mousetrap.bind($scope, "* a", vm.selectAll, "Select All");
            Mousetrap.bind($scope, "* n", vm.deselectAll, "Deselect All");
            Mousetrap.bind($scope, "* 1", function() {
                selectBySeverity(1);
            }, "Select All Severity 1 Events");
            Mousetrap.bind($scope, "* 2", function() {
                selectBySeverity(2);
            }, "Select All Severity 2 Events");
            Mousetrap.bind($scope, "* 3", function() {
                selectBySeverity(3);
            }, "Select All Severity 3 Events");
            Mousetrap.bind($scope, "e", vm.archiveSelected,
                "Archive Selected Events");
            Mousetrap.bind($scope, "#", vm.deleteSelected,
                "Delete Selected Events");
            Mousetrap.bind($scope, "x", function() {
                toggleSelect();
            }, "Select Event");
            Mousetrap.bind($scope, "s", function() {
                vm.toggleStar(getActiveEvent());
            }, "Toggle 'Star'");
            Mousetrap.bind($scope, "/", function(e) {
                e.preventDefault();
                $("#search-form-input").focus();
                $("#search-form-input").select();
            }, "Search");
            Mousetrap.bind($scope, "g 1", function() {
                vm.gotoView("flat");
            }, "Go to Flat View");
            Mousetrap.bind($scope, "g 2", function() {
                vm.gotoView("signature");
            }, "Go to Grouped by Signature View");
            Mousetrap.bind($scope, "g 3", function() {
                vm.gotoView("signature+src");
            }, "Go to Grouped by Signature/Source View");
            Mousetrap.bind($scope, "o", function() {
                vm.open(getActiveEvent());
            }, "Open Event");

            vm.filters = [{term: {event_type: "alert"}}];

            if (baseUrl == "/inbox") {
                vm.filters.push({term: {tags: "inbox"}});
            }
            else if (baseUrl == "/starred") {
                vm.filters.push({term: {tags: "starred"}});
            }

            load();

        })();

    }
    ;

})();

