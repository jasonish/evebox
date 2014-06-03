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

app.directive("keyTable", function () {

    directive = {};

    directive.restrict = "A";

    directive.scope = {
        rows: "=",
        activeRowIndex: "="
    };

    directive.controller = function ($scope, Keyboard, Util, $element) {

        keyTableScope = $scope;

        $scope.$element = $element;
        $scope.Keyboard = Keyboard;
        $scope.activeRowIndex = 0;

        var scrollToView = function () {
            Util.scrollElementIntoView(
                angular.element(
                    $element).find("tr").eq($scope.activeRowIndex));
        };

        Keyboard.scopeBind($scope, "j", function () {
            console.log($scope.rows.length);
            console.log(angular.element($element).find("tr").length);
            console.log(angular.element($element).find("tbody").length);
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