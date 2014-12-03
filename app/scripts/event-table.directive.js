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

    /**
     * Adds some keyboard controls to a table.
     *
     * Only one event-table can exist on a given page at this time.
     */
    angular.module("app").directive("eventTable", ["Mousetrap", function (Mousetrap) {

        var directive = {
            restrict: "A",
            scope: {
                rows: "=eventTableRows",
                activeRowIndex: "=eventTableActiveRowIndex"
            }
        };

        directive.link = function (scope, element, attrs) {

            var scrollIntoView = function () {
                if (scope.activeRowIndex == 0) {
                    window.scrollTo(0, 0);
                }
                else {
                    var element = $("table[event-table] tbody").eq(scope.activeRowIndex);
                    if (element.offset().top > window.pageYOffset
                        + window.innerHeight) {
                        window.scrollTo(0,
                            element.offset().top - (window.innerHeight * .25));
                    }
                    else if (element.offset().top < window.pageYOffset) {
                        window.scrollTo(0,
                            element.offset().top - (window.innerHeight * .75));
                    }
                }
            };

            Mousetrap.bind(scope, "j", function () {
                if (scope.activeRowIndex < scope.rows.length - 1) {
                    scope.activeRowIndex++;
                    scrollIntoView();
                }
            });

            Mousetrap.bind(scope, "k", function () {
                if (scope.activeRowIndex > 0) {
                    scope.activeRowIndex--;
                    scrollIntoView();
                }
            });

            Mousetrap.bind(scope, "G", function () {
                scope.activeRowIndex = scope.rows.length - 1;
                scrollIntoView();
            });

            Mousetrap.bind(scope, "H", function () {
                scope.activeRowIndex = 0;
                scrollIntoView();
            });

        };

        return directive;

    }]);

})();
