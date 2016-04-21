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

import "angular-ui-router";
import "angular-touch";

import * as appEvents from "./app-events";

var app = angular.module("app", ["ngTouch", "ui.router"]);

angular.module("app").config(config);

config.$inject = ["$stateProvider", "$urlRouterProvider"];

function config($stateProvider, $urlRouterProvider) {

    $urlRouterProvider.otherwise("/inbox");

    $stateProvider.state("alerts", {
        url: "/alerts?q",
        template: "<alerts></alerts>",
        data: {
            mode: "alerts"
        }
    });

    $stateProvider.state("escalated", {
        url: "/escalated?q",
        template: "<alerts></alerts>",
        data: {
            mode: "escalated"
        }
    });

    $stateProvider.state("inbox", {
        url: "/inbox?q",
        template: "<alerts></alerts>",
        data: {
            mode: "inbox"
        }
    });

    $stateProvider.state("events", {
        url: "/events?q&timeStart&timeEnd",
        template: "<events></events>"
    });

    $stateProvider.state("event", {
        url: "/event/:id",
        template: "<event></event>"
    });

}

angular.module("app").run(run);

run.$inject = ["$rootScope"];

function run($rootScope) {
    $(window).resize(function () {
        $rootScope.$broadcast(appEvents.WINDOW_RESIZE);
    });
}

angular.element(document).ready(function () {
    var initInjector = angular.injector(["ng"]);
    var $http = initInjector.get("$http");
    $http.get("/api/config").then(response => {
        app.constant("rawConfig", response.data);
        angular.bootstrap(document, ["app"]);
    });
});