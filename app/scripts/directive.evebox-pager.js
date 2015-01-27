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

    angular.module("app").directive("eveboxPager",
        function (Config, printf) {

            var directive = {
                restrict: "A",
                templateUrl: "templates/evebox-pager.html",
                scope: {
                    pageSize: "=",
                    currentPage: '=',
                    onPageChange: '=',
                    totalItems: '=',
                    items: '='
                }
            };

            directive.link = function (scope, element, attrs) {

                scope.$watch('items.length', function () {

                    if (scope.items) {
                        scope.lastPage =
                            Math.floor(scope.totalItems / scope.pageSize) + 1;
                        scope.pageFirstItem =
                            (scope.pageSize * (scope.currentPage - 1)) + 1;
                        scope.pageLastItem =
                            scope.pageFirstItem + scope.items.length - 1;
                    }

                });

            };

            directive.controller = function ($scope) {

                $scope.gotoPage = function (page) {
                    console.log("eveboxPagerNew.gotoPage: " + page);
                    switch (page) {
                        case "first":
                            $scope.onPageChange(1);
                            break;
                        case "previous":
                            if ($scope.currentPage > 1) {
                                $scope.onPageChange($scope.currentPage - 1);
                            }
                            break;
                        case "next":
                            if ($scope.currentPage < $scope.lastPage) {
                                $scope.onPageChange($scope.currentPage + 1);
                            }
                            break;
                        case "last":
                            $scope.onPageChange($scope.lastPage);
                        default:
                            break;
                    }
                }

            };

            return directive;

        });

})();
