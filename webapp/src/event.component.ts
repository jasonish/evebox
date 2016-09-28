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

import {Component, OnInit, OnDestroy} from "@angular/core";
import {Location} from "@angular/common";
import {ActivatedRoute, Router} from "@angular/router";
import {ElasticSearchService, AlertGroup} from "./elasticsearch.service";
import {ApiService} from "./api.service";
import {EventServices} from "./eventservices.service";
import {EventService} from "./event.service";
import {MousetrapService} from "./mousetrap.service";
import {EveboxSubscriptionService} from "./subscription.service";

/**
 * Component to show a single event.
 */
@Component({
    template: require("./event.component.html"),
})
export class EventComponent implements OnInit, OnDestroy {

    private eventId:string;
    private alertGroup:AlertGroup;
    private event:any = {};
    private params:any = {};
    private flows:any[] = [];

    private servicesForEvent:any[] = []

    constructor(private route:ActivatedRoute,
                private router:Router,
                private elasticSearchService:ElasticSearchService,
                private api:ApiService,
                private eventServices:EventServices,
                private location:Location,
                private eventService:EventService,
                private mousetrap:MousetrapService,
                private ss:EveboxSubscriptionService) {
    }

    reset() {
        this.eventId = undefined;
        this.alertGroup = undefined;
        this.event = {};
        this.params = {};
        this.flows = [];
    }

    setup() {
        console.log("setup");
        this.servicesForEvent = this.eventServices.getServicesForEvent(this.event);
    }

    ngOnInit() {

        let alertGroup = this.eventService.popAlertGroup();

        this.ss.subscribe(this, this.route.params, (params:any) => {

            this.reset();

            this.params = params;
            this.eventId = params.id;

            if (alertGroup && this.eventId == alertGroup.event._id) {
                this.alertGroup = alertGroup;
                this.event = this.alertGroup.event;
                if (this.event._source.event_type != "flow") {
                    this.findFlow(this.event);
                }
                this.setup();
            }
            else {
                this.refresh();
            }

        });

        this.mousetrap.bind(this, "u", () => this.goBack());
        this.mousetrap.bind(this, "e", () => this.archiveEvent());
        this.mousetrap.bind(this, "f8", () => this.archiveEvent());

    }

    ngOnDestroy() {
        this.ss.unsubscribe(this);
        this.mousetrap.unbind(this);
    }

    eventToPcap(what:any) {
        this.api.eventToPcap(what, this.event._source);
    }

    goBack() {
        this.location.back();
    }

    showArchiveButton() {
        return this.event._source.event_type == "alert" &&
            this.event._source.tags.indexOf("archived") == -1;
    }

    sessionSearch() {
        let q = `+alert.signature_id:${this.event._source.alert.signature_id}`;
        q += ` +src_ip.raw:"${this.event._source.src_ip}"`;
        q += ` +dest_ip.raw:"${this.event._source.dest_ip}"`;

        if (this.params && this.params.referer == "/inbox") {
            q += ` -tags:archived`;
        }

        console.log(q);

        this.router.navigate(["/events", {q: q}]);
    }

    archiveEvent() {
        if (this.alertGroup) {
            this.elasticSearchService.archiveAlertGroup(this.alertGroup);
            this.alertGroup.event._source.tags.push("archived");
        }
        else {
            this.elasticSearchService.archiveEvent(this.event);
        }
        this.location.back();
    }

    escalateEvent() {
        if (this.alertGroup) {
            this.elasticSearchService.escalateAlertGroup(this.alertGroup);
            this.alertGroup.escalatedCount = this.alertGroup.count;
        }
        else {
            console.log("Escalating single event.");
            this.elasticSearchService.escalateEvent(this.event);
        }
    }

    deEscalateEvent() {
        if (this.alertGroup) {
            this.elasticSearchService._removeEscalatedStateFromAlertGroup(this.alertGroup);
            this.alertGroup.escalatedCount = 0;
        }
        else {
            this.elasticSearchService.removeTagsFromEventSet([this.event], ["escalated", "evebox.escalated"]);
        }
        this.location.back();
    }

    isEscalated() {

        if (this.alertGroup) {
            if (this.alertGroup.escalatedCount == this.alertGroup.count) {
                return true;
            }
        }

        if (!this.event._source.tags) {
            return false;
        }

        if (this.event._source.tags.indexOf("escalated") > -1) {
            return true;
        }

        if (this.event._source.tags.indexOf("evebox.escalated") > -1) {
            return true;
        }

        return false;
    }

    findFlow(event:any) {

        if (!event.flow_id) {
            console.log("Unable to find flow for event, event does not have a flow id.");
            return;
        }

        let query = {
            query: {
                filtered: {
                    filter: {
                        and: [
                            {exists: {field: "event_type"}},
                            {term: {event_type: "flow"}},
                            {term: {flow_id: event._source.flow_id}},
                            {term: {"proto.raw": event._source.proto}},
                            {
                                or: [
                                    {term: {"src_ip.raw": event._source.src_ip}},
                                    {term: {"src_ip.raw": event._source.dest_ip}},
                                ]
                            },
                            {
                                or: [
                                    {term: {"dest_ip.raw": event._source.src_ip}},
                                    {term: {"dest_ip.raw": event._source.dest_ip}},
                                ]
                            },
                            {
                                range: {
                                    "flow.start": {
                                        lte: event._source.timestamp,
                                    }
                                }
                            },
                            {
                                range: {
                                    "flow.start": {
                                        lte: event._source.timestamp,
                                    }
                                }
                            }
                        ]
                    }
                }
            }
        };
        this.elasticSearchService.search(query).then((response:any) => {
            if (response.hits.hits.length > 0) {
                this.flows = response.hits.hits;
            }
            else {
                console.log("No flows found for event.");
            }
        })
    }

    refresh() {
        this.elasticSearchService.getEventById(this.eventId)
            .then((response:any) => {
                this.event = response;
                if (this.event._source.event_type != "flow") {
                    this.findFlow(response);
                }
                this.setup();
            });
    }
}