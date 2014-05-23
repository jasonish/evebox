/* A sample test... */

'use strict';

describe('Controller: MainController', function () {

    var scope;
    var controller;

    beforeEach(module('app'));
    beforeEach(module('ui.bootstrap')); // Add this line

    beforeEach(inject(function ($rootScope, $controller) {
        scope = $rootScope.$new();
        controller = $controller;
    }));

    it("should assign message to hello world", function () {
        controller("MainController", {$scope: scope});
        expect(scope.page).toBe(1);
    });

});
