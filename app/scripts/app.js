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

var app = angular.module("app", [
    "ngRoute",
    "ngResource",
    "ngSanitize",
    "ngAnimate",
    "ui.bootstrap",
    "ui.bootstrap.modal",
    "ui.bootstrap.accordion"
]);

(function () {

    angular.module("app").config(function ($routeProvider, $locationProvider) {

        /* Individual event. */
        $routeProvider.when("/event/:id", {
            controller: "EventController",
            controllerAs: "vm",
            templateUrl: "templates/event.html"
        });

        /* All events view. */
        $routeProvider.when("/events", {
            controller: "EventsController",
            controllerAs: "vm",
            templateUrl: "templates/events.html"
        });

        /* Inbox. */
        $routeProvider.when("/inbox/:view", {
            controller: "NewAlertsController",
            controllerAs: "vm",
            templateUrl: "templates/alerts.html",
            resolve: {
                baseUrl: function () {
                    return "/inbox";
                }
            }
        });
        $routeProvider.when("/inbox", {
            controller: "RedirectController",
            template: ""
        });

        /* Starred. */
        $routeProvider.when("/starred/:view", {
            controller: "NewAlertsController",
            controllerAs: "vm",
            templateUrl: "templates/alerts.html",
            resolve: {
                baseUrl: function () {
                    return "/starred";
                }
            }
        });
        $routeProvider.when("/starred", {
            redirectTo: "/starred/flat"
        });

        /* Alerts. */
        $routeProvider.when("/alerts/:view", {
            controller: "NewAlertsController",
            controllerAs: "vm",
            templateUrl: "templates/alerts.html",
            resolve: {
                baseUrl: function () {
                    return "/alerts";
                }
            }
        });
        $routeProvider.when("/alerts", {
            redirectTo: "/alerts/flat"
        });

        $routeProvider.when("/help", {
            templateUrl: "templates/help.html",
            controller: "HelpController",
            controllerAs: "vm"
        });

        /* Default to inbox. */
        $routeProvider.otherwise({redirectTo: "/inbox"});
    });

    angular.module("app").controller("RedirectController",
        function ($location, Config) {

            if ($location.path() == "/inbox") {
                $location.path(
                    "/inbox/" + (Config.defaultInboxAggregation || "flat"));
            }

        });

    /**
     * Add .startsWith to the string type.
     */
    if (typeof String.prototype.startsWith != 'function') {
        // see below for better implementation!
        String.prototype.startsWith = function (str) {
            return this.indexOf(str) == 0;
        };
    }

})();

