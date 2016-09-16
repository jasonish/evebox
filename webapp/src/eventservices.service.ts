/* Copyright (c) 2014-2016 Jason Ish
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

import {Injectable} from "@angular/core";
import {ConfigService} from "./config.service";

export class CustomEventService {

    private name:string;
    private url:string;
    private target:string;
    private eventTypes:string[] = [];

    constructor(config:any) {

        this.name = config.name;
        this.url = config.url;
        this.target = config.target || "_top";

        // If "new", set to "_blank.".
        if (this.target == "new") {
            this.target = "_blank";
        }

        if (config["event-types"]) {
            this.eventTypes = config["event-types"];
        }

    }

    isValidForEvent(event:any) {
        if (this.eventTypes.length == 0) {
            return true;
        }
        return this.eventTypes.indexOf(event._source.event_type) > -1;
    }

    getTarget() {
        return this.target;
    }

    getUrl(event:any) {
        return this.resolveUrl(this.url, event);
    }

    getField(name:string, event:any) {
        var parts = name.split(".");
        var node = event._source;

        for (var i = 0; i < parts.length; i++) {
            if (!node[parts[i]]) {
                return "";
            }
            node = node[parts[i]];
        }

        return node;
    }

    resolveUrl(url:string, event:any) {
        while (true) {
            var match = url.match(/{{(.*?)}}/);
            if (!match) {
                break;
            }

            var replacement = "";

            switch (match[1]) {
                case "raw":
                    replacement = encodeURIComponent(JSON.stringify(event._source));
                    break;
                default:
                    replacement = this.getField(match[1], event);
                    break;
            }

            url = url.replace(match[0], replacement);
        }
        return url;
    }
}

@Injectable()
export class EventServices {

    private services:any[] = [];

    constructor(private configService:ConfigService) {

        /* The config may already be here... */
        let config = configService.getConfig()
        if (config) {
            this.initServices(config)
        }

        this.configService.subscribe((config:any) => {
            this.initServices(config);
        })

    }

    initServices(config:any) {

        this.services = [];

        if (!config["event-services"]) {
            console.log("No configured event services.");
            return;
        }

        config["event-services"].forEach((serviceConfig:any) => {
            if (serviceConfig.enabled) {
                let service = new CustomEventService(serviceConfig);
                this.services.push(service);
            }
        })
    }

    getServicesForEvent(event:any) {
        let services = this.services.filter((service:CustomEventService) => {
            return service.isValidForEvent(event);
        });
        return services;
    }
}