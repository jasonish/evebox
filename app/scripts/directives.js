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

app.directive("autoBlur", function() {
    return {

        restrict: "A",

        link: function(scope, element, attr) {
            element.find(":input").bind("click", function() {
                $(this).blur();
            });
            element.bind("click", function() {
                element.blur();
            })
        }

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

app.directive("duration", function($interval) {

    return {
        restrict: "AE",

        scope: {
            timestamp: "="
        },

        template: "{{duration}}",

        link: function(scope, element, attrs) {

            // 6 seconds...  One second shows a noticeable increase in CPU.
            var updateInterval = 6000;

            var intervalId;

            element.on("$destroy", function() {
                $interval.cancel(intervalId);
            });

            var updateDuration = function() {
                var duration = moment(scope.timestamp) - moment();
                scope.duration = moment.duration(duration).humanize(true);
            };
            updateDuration();

            intervalId = $interval(function() {
                updateDuration();
            }, updateInterval);

        }
    }

});