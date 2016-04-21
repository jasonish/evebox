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

import * as evebox from "./evebox";
import template from "./event-template.html";

(function () {

    angular.module("app").directive("event", event);

    event.$inject = ["$anchorScroll", "$state", "EventRepository", "Keyboard",
        "StateService", "EveBoxApi", "EventServices"];

    function event($anchorScroll, $state, EventRepository, Keyboard,
                   StateService, EveBoxApi, EventServices) {

        return {
            restrict: "AE",
            scope: {},
            template: template,
            controller: ["$scope", controller],
            controllerAs: "vm",
            bindToController: true,
            link: link
        };

        function controller($scope) {

            let vm = this;

            vm.event = StateService.get($state.params.id);
            vm.showBackButton = true;
            vm.EventServices = EventServices;

            if (vm.event != undefined) {
                console.log("Found event in state.");
                setup(vm.event);
            }
            else {
                EventRepository.getEventById($state.params.id).then(
                    response => {
                        vm.event = response;
                        if (vm.event) {
                            setup(vm.event);
                        }
                    });
            }

            vm.showEscalateButton = () => {
                if (evebox.eventType(vm.event) == "alert") {
                    if (vm.event.count != vm.event.escalated) {
                        return true;
                    }
                }
                return false;
            };

            vm.showArchiveButton = function() {
                if (evebox.eventType(vm.event) == "alert") {
                    if (evebox.getTags(vm.event).indexOf("archived") < 0) {
                        return true;
                    }
                }
                return false;
            };

            vm.goBack = function () {
                window.history.back();
            };

            vm.archiveEvent = function () {

                // Optimistically archive the event (don't wait for success).
                EventRepository.submitArchiveEvent(vm.event);

                // Mark as archived and return.
                evebox.addTag(vm.event, "archived");
                vm.goBack();
            };

            vm.escalateEvent = () => {
                EventRepository.addEscalated(vm.event).then(() => {
                    vm.event.escalated = vm.event.count;
                });
            };

            vm.downloadPcap = function(what) {
                console.log(what);
                EveBoxApi.downloadPcap(what, vm.event.newest._source);
            };

            vm.getSessionSearchUrl = function () {
                let event = vm.event.newest._source;
                let queryString = `%2balert.signature.raw:"${event.alert.signature}"` +
                    ` %2bsrc_ip.raw:"${event.src_ip}"` +
                    ` %2bdest_ip.raw:"${event.dest_ip}"`;

                if (!event.tags || event.tags.indexOf("archived") < 0) {
                    queryString += ` -tags:archived`;
                }

                return "#/events?q=" + queryString;
            };

            Keyboard.bind($scope, "u", () => {
                vm.goBack();
            });

            Keyboard.bind($scope, "e", () => {
                vm.archiveEvent();
            });

            function setup(event) {
                // As the actual contents of the vent is buried down a little,
                // pull it up into the model for easier access from the template.
                vm.source = vm.event.newest._source;
            }
        }

        function link() {
            $anchorScroll();
        }

    }

})();
