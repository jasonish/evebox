/*
 * Copyright (c) 2014 Jason Ish
 * All rights reserved.
 */

/*
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

app.directive("eveboxIpAddressServices", function () {

    return {

        restrict: "EA",

        templateUrl: "templates/ip-address-services.html",

        scope: {
            address: "=address"
        }

    };

});

app.directive("eveboxNotificationMessage", function() {

    return {

        restrict: "EA",

        controller: function($scope, NotificationMessageService) {

            x = NotificationMessageService;

            $scope.NotificationMessageService = NotificationMessageService;

        }
    };

});

app.directive("eveboxSearchForm", function (Keyboard) {

    return {

        restrict: "EA",

        templateUrl: "templates/search-form.html",

        controller: function ($scope) {

            $scope.$on("$destroy", function () {
                Keyboard.resetScope($scope);
            });

            Keyboard.scopeBind($scope, "/", function (e) {
                e.preventDefault();
                $("#user-query-input").focus();
                $("#user-query-input").select();
            });

        }

    };

});

app.directive("eveboxPager", function () {

    return {
        restrict: "EA",

        templateUrl: "templates/pager.html",

        scope: {
            rows: "=eveboxPagerRows",
            page: "=eveboxPagerPage",
            change: "&eveboxPagerChangePage",
            querySize: "=eveboxPagerQuerySize",
            total: "=eveboxPagerTotal",
            size: "@eveboxPagerSize"
        }, // scope.

        controller: function ($scope, Keyboard) {

            $scope.gotoPage = function (what) {
                var last = Math.floor($scope.total / $scope.querySize) + 1;

                switch (what) {
                    case "first":
                        $scope.page = 1;
                        break;
                    case "prev":
                        if ($scope.page > 1) {
                            $scope.page--;
                        }
                        break;
                    case "next":
                        if ($scope.page < last) {
                            $scope.page++;
                        }
                        break;
                    case "last":
                        $scope.page = last;
                        break;
                }

                $scope.change()($scope.page);
            };

            $scope.$on("$destroy", function () {
                Keyboard.resetScope($scope);
            });

            Keyboard.scopeBind($scope, "{", function () {
                $scope.$apply(function () {
                    $scope.gotoPage("first");
                })
            });

            Keyboard.scopeBind($scope, "<", function () {
                $scope.$apply(function () {
                    $scope.gotoPage("prev");
                });
            });

            Keyboard.scopeBind($scope, ">", function () {
                $scope.$apply(function () {
                    $scope.gotoPage("next");
                });
            });

            Keyboard.scopeBind($scope, "}", function () {
                $scope.$apply(function () {
                    $scope.gotoPage("last");
                })
            });

        }, // controller.

        link: function (scope, element, attributes) {
            switch (scope.size) {
                case "sm":
                    angular.element(element).find(".btn-group").addClass("btn-group-sm");
                    break;
            }
        } //link.
    };

});

app.directive("keyTable", function () {

    directive = {
        restrict: "A"
    };

    directive.scope = {
        rows: "=keyTableRows",
        activeRowIndex: "=keyTableActiveRowIndex"
    };

    directive.controller = function ($scope, Keyboard, Util, $element) {

        console.log("keyTable");

        keyTableScope = $scope;

        $scope.$element = $element;
        $scope.Keyboard = Keyboard;
        $scope.activeRowIndex = 0;

        var scrollToView = function () {

            var rowIndexClass = "row-index-" + $scope.activeRowIndex;
            var row = angular.element($element).find("." + rowIndexClass);
            if (row.hasClass(rowIndexClass)) {
                Util.scrollElementIntoView(row);
            }
            else {
                Util.scrollElementIntoView(
                    angular.element(
                        $element).find("tr").eq($scope.activeRowIndex));
            }
        };

        Keyboard.scopeBind($scope, "j", function () {
            $scope.$apply(function () {
                if ($scope.activeRowIndex < $scope.rows.length - 1) {
                    $scope.activeRowIndex++;
                }
                scrollToView();
            });
        });

        Keyboard.scopeBind($scope, "k", function () {
            $scope.$apply(function () {
                if ($scope.activeRowIndex > 0) {
                    $scope.activeRowIndex--;
                }
                scrollToView();
            });
        });

        Keyboard.scopeBind($scope, "H", function (e) {
            $scope.$apply(function () {
                $(window).scrollTop(0);
                $scope.activeRowIndex = 0;
            });
        });

        Keyboard.scopeBind($scope, "G", function (e) {
            $scope.$apply(function () {
                $(window).scrollTop($(document).height())
                $scope.activeRowIndex = $scope.rows.length - 1;
            });
        });

    };

    return directive;
});