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

(function () {

    angular.module("app").controller("EventsController", EventsController);

    /**
     * Controller for the all events view.
     */
    function EventsController($scope, $routeParams, $location, Mousetrap,
        EventRepository, $timeout) {

        var vm = this;

        vm.activeRowIndex = 0;
        vm.openEventIndex = - 1;
        vm.query = $routeParams.q || "";
        vm.page = parseInt($routeParams.page) || 1;
        vm.loading = true;

        /**
         * Open the event (inline).
         */
        vm.open = function (event) {
            var index = _.indexOf(vm.events, event);
            if (vm.openEventIndex == index) {
                vm.openEventIndex = - 1;
            }
            else {
                vm.openEventIndex = index;
                vm.activeRowIndex = index;

                /* XXX Scrolls the open event to the top. */
                $timeout(function () {
                    var element = $("table[event-table] tbody")
                        .eq(vm.openEventIndex);
                    $("html, body").animate(
                        {scrollTop: element.offset().top}, 100);

                });
            }

        };

        vm.refresh = function () {
            doSearch();
        };

        vm.gotoPage = function (page) {
            $location.search("page", page);
        };

        var doSearch = function () {
            vm.loading = true;
            EventRepository.getEvents({
                query: vm.query,
                page: vm.page
            }).then(
                // Success.
                function (result) {
                    vm.result = result;
                    vm.events = result.events;
                    vm.loading = false;
                },
                // Error.
                function (result) {

                }
            )
        };

        var getActiveEvent = function () {
            return vm.events[vm.activeRowIndex];
        };

        vm.submitSearchForm = function () {
            $location.search("q", vm.query);
        };

        vm.toggleStar = function (event) {
            if (! event.count) {
                EventRepository.toggleStar(event);
            }
        };

        var initKeybindings = function () {

            Mousetrap.bind($scope, "/", function (e) {
                e.preventDefault();
                $("#user-query-input").focus();
                $("#user-query-input").select();
            });
            Mousetrap.bind($scope, "o", function () {
                vm.open(getActiveEvent());
            });
            Mousetrap.bind($scope, "r", function () {
                vm.refresh();
            });
            Mousetrap.bind($scope, "s", function() {
                vm.toggleStarr(getActiveEvent());
            });

        };

        // Init.
        (function () {
            initKeybindings();
            doSearch();
        })();

    };

})();