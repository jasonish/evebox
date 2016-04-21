/* Copyright (c) 2016 Jason Ish
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

(function () {

    function getField(name, event) {
        var parts = name.split(".");
        var node = event._source;

        for (var i = 0; i < parts.length; i++) {
            if (!node[parts[i]]) {
                return null;
            }
            node = node[parts[i]];
        }

        return node;
    }
    
    function resolveUrl(url, event) {
        while (true) {
            var match = url.match(/{{(.*?)}}/);
            if (!match) {
                break;
            }

            var replacement = getField(match[1], event);

            url = url.replace(match[0], replacement);
        }
        return url;
    }

    class CustomEventService {

        constructor(args) {
            console.log("Initializing custom event service: args=" +
                JSON.stringify(args));
            if (!args.name) {
                throw "no name provided";
            }
            this.name = args.name;
            this.url = args.url;

            if (args["event-types"]) {
                this.eventTypes = args["event-types"];
            }
        }

        isValidForEvent(event) {
            if (!this.eventTypes) {
                return true;
            }
            return this.eventTypes.indexOf(event._source.event_type) > -1;
        }

        getUrl(event) {
            return resolveUrl(this.url, event);
        }

    }

    class EventServices {

        constructor(Config) {

            // Register known event services.
            this.availableServices = {
                "custom": CustomEventService
            };

            this.services = [];

            var config = Config.getEventServices();

            _.forEach(config, (entry) => {

                if (this.availableServices[entry.type] == undefined) {
                    console.log("Unknown event service type: " + entry.type);
                    return;
                }

                if (!entry.enabled) {
                    return;
                }

                try {
                    var eventService = new this.availableServices[entry.type](entry);
                    this.services.push(eventService);
                }
                catch (err) {
                    console.log(`Failed to initialize event service of type ${entry.type}: ${err}.`);
                }

            });
        }

        getServices() {
            return this.services;
        }

        getServicesForEvent(event) {
            var services = _.filter(this.services, service => {
                return service.isValidForEvent(event);
            });
            return services;
        }

    }

    EventServices.$inject = ["Config"];

    angular.module("app").service("EventServices", EventServices);

})();