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

(function() {

    angular.module("app").directive("keyboardTable", keyboardTable);

    keyboardTable.$inject = ["Keyboard"];

    function keyboardTable(Keyboard) {

        controller.$inject = ["$scope", "$element"];

        function controller($scope, $element) {

            var vm = this;

            function rowCount() {
                return $element.find("tbody").children().length;
            }

            function scrollToActiveRow() {
                // Ensure that when the first item is active, the top of the
                // page is visible.
                if (vm.activeRow == 0) {
                    window.scrollTo(0, 0);
                }
                else {
                    let activeElement = $element.find(
                        "tbody").children()[vm.activeRow];
                    if (activeElement) {
                        jQuery(window).scrollTop(activeElement.offsetTop - 50);
                    }
                }
            }

            // Watch the row count, we can update the activeRow if the current
            // active row dissappears.
            //
            // Disabled for now to see if this is the cause of a flickering
            // issue.
            //$scope.$watch('vm.rows.length', () => {
            //    if (vm.rows) {
            //        // Wrap it setTimeout so it happens after Angular does
            //        // its stuff with the DOM.
            //        setTimeout(scrollToActiveRow, 0);
            //    }
            //});

            // Watch the active row. It may be updated outside the control of
            // this directive, for example, after a refresh where it will be
            // set back to 0.
            $scope.$watch("vm.activeRow", () => {
                setTimeout(scrollToActiveRow, 0);
            });

            Keyboard.bind($scope, "j", (e) => {
                if (this.activeRow < rowCount() - 1) {
                    this.activeRow++;
                    scrollToActiveRow();
                }
            });

            Keyboard.bind($scope, "G", () => {
                this.activeRow = rowCount() - 1;
                scrollToActiveRow();
            });

            Keyboard.bind($scope, "H", () => {
                this.activeRow = 0;
                scrollToActiveRow();
            });

            Keyboard.bind($scope, "k", (e) => {
                if (this.activeRow > 0) {
                    this.activeRow--;
                    scrollToActiveRow();
                }
            });

            if (vm.onRowOpen) {
                Keyboard.bind($scope, "o", (e) => {
                   vm.onRowOpen(this.activeRow);
                });
            }

            scrollToActiveRow();
        }

        return {
            restrict: "A",
            scope: {
                activeRow: "=",
                rows: "=",
                onRowOpen: "="
            },
            controller: controller,
            controllerAs: "vm",
            bindToController: true
        }
    }

})();
