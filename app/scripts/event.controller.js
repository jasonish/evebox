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

    angular.module("app").controller("EventController", EventController);

    function EventController($scope, $routeParams, ElasticSearch, Util,
        $anchorScroll, Mousetrap) {

        var vm = this;
        this.Util = Util;
        var eventId = $routeParams.id;

        function formatEvent(hit) {
            hit.__titleClass = "alert-info";

            if (hit._source.alert) {
                hit.__title = hit._source.alert.signature;
                hit.__titleClass =
                    Util.severityToBootstrapClass(hit._source.alert.severity,
                        "alert-");
            }
            else if (hit._source.dns) {
                hit.__title = Util.printf("{}: {}",
                    hit._source.event_type.toUpperCase(),
                    hit._source.dns.rrname);
                hit.__titleClass = "alert-info";
            }
            else if (hit._source.tls) {
                hit.__title = Util.printf("{}: {}",
                    hit._source.event_type.toUpperCase(),
                    hit._source.tls.subject);
                hit.__titleClass = "alert-info";
            }
            else if (hit._source.http) {
                hit.__title = Util.printf("{}: {} {}",
                    hit._source.event_type.toUpperCase(),
                    hit._source.http.http_method,
                    hit._source.http.hostname);
            }
            else {
                hit.__title = hit._source.event_type.toUpperCase();
                hit.__titleClass = "alert-info";
            }

            if (!hit.__titleClass) {
                hit.__titleClass = "alert-info";
            }
        }

        // Init.
        (function() {

            $anchorScroll();

            Mousetrap.bind($scope, "u", function() {
                window.history.back();
            });

            ElasticSearch.searchEventById(eventId)
                .success(function(response) {
                    $scope.hits = response.hits;
                    _.forEach($scope.hits.hits, formatEvent);
                });

        })();

    }

})();