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

(function() {

    'use strict';

    app.controller("NavBarController", function($routeParams, $scope, $modal,
        $location, EventRepository, $timeout, Mousetrap) {

        $scope.$routeParams = $routeParams;
        $scope.$location = $location;

        $scope.openConfig = function() {
            $modal.open({
                templateUrl: "templates/config.html",
                controller: "ConfigController as vm"
            });
        };

        $scope.openHelp = function() {
            $modal.open({
                templateUrl: "templates/help.html",
                controller: "HelpController as vm",
                size: "lg"
            });
        };

        Mousetrap.bind($scope, "g i", function(e) {
            $location.url("/inbox");
        }, "Go to Inbox");

        Mousetrap.bind($scope, "g s", function(e) {
            $location.url("/starred")
        }, "Go to Starred");

        Mousetrap.bind($scope, "g a", function(e) {
            $location.url("/alerts");
        }, "Go to Alerts");

        Mousetrap.bind($scope, "g o", function() {
            $("#other-menu-dropdown-toggle").dropdown('toggle');
        }, "Go to Other");

        Mousetrap.bind($scope, "g c", function(e) {
            $scope.openConfig();
        }, "Go to Configuration");

        Mousetrap.bind($scope, "?", function(e) {
            $scope.openHelp();
        }, "Show Help");
    });

    angular.module("app").controller("HelpController", HelpController);

    function HelpController(Mousetrap) {
        var vm = this;
        vm.bindings = Mousetrap.bindings;
        vm.sortedKeys = _.sortBy(_.keys(Mousetrap.bindings));
    };

})();
