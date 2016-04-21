/* Copyright (c) 2014-2015 Jason Ish
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

import angular from "angular";

import * as appEvents from "./app-events";
import template from "./topnav-template.html";

(function () {

    angular.module("app").factory("TopNavService", TopNavService);

    // Config is specified here as a bit of a hack, to force it to be
    // intitialized before its needed.
    TopNavService.$inject = ["$rootScope", "$location", "Config"];

    function TopNavService($rootScope, $location, Config) {

        let DEFAULT_TIME_RANGE = "24h";

        let service = {
            timeRange: DEFAULT_TIME_RANGE,
            timeRangeEnabled: true
        };

        // Always re-enable the time range on route change. The controllers
        // can set this to false as needed.
        $rootScope.$on("$stateChangeStart", () => {
            service.timeRangeEnabled = true;
        });

        service.timeRange = $location.search().timeRange || DEFAULT_TIME_RANGE;

        return service;

    }

    angular.module("app").directive("appTopNav", appTopNav);

    appTopNav.$inject =
        ["$rootScope", "TopNavService", "Keyboard", "EventRepository",
            "StateService"];

    function appTopNav($rootScope, TopNavService, Keyboard, EventRepository,
                       StateService) {

        controller.$inject = ["$scope", "$state"];

        function controller($scope, $state) {

            var vm = this;

            vm.$state = $state;
            vm.EventRepository = EventRepository;
            vm.TopNavService = TopNavService;

            $scope.$watch('vm.TopNavService.timeRange', function () {
                $rootScope.$broadcast(appEvents.TIMERANGE_CHANGED,
                    vm.timeRange);
            });

            vm.showHelp = () => {
                console.log("Showing help.");
                $rootScope.$broadcast("evebox.showHelp");
            };

            vm.gotoState = (state) => {
                StateService.clear();
                $state.go(state, {q: ""});
            };

            Keyboard.bind($scope, "g i", () => {
                vm.gotoState("inbox")
            });

            Keyboard.bind($scope, "g x", () => {
                vm.gotoState("escalated");
            });

            Keyboard.bind($scope, "g a", () => {
                vm.gotoState("alerts");
            });

            Keyboard.bind($scope, "g e", () => {
                vm.gotoState("events");
            });

            Keyboard.bind($scope, "?", vm.showHelp);
        }

        return {
            restrict: "AE",
            template: template,
            scope: {},
            controller: controller,
            controllerAs: "vm",
            bindToController: true
        }

    }

})();

