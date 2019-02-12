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

import {Component, OnDestroy, OnInit} from "@angular/core";
import {Location} from "@angular/common";
import {ActivatedRoute, Router} from "@angular/router";
import {AlertGroup, ElasticSearchService} from "../elasticsearch.service";
import {ApiService} from "../api.service";
import {EventServices} from "../eventservices.service";
import {EventService} from "../event.service";
import {MousetrapService} from "../mousetrap.service";
import {EveboxSubscriptionService} from "../subscription.service";
import {loadingAnimation} from "../animations";
import {ToastrService} from "../toastr.service";
import {FEATURE_COMMENTS} from "../app.service";
import {ConfigService} from "../config.service";

/**
 * Component to show a single event.
 */
@Component({
    templateUrl: "./event.component.html",
    animations: [
        loadingAnimation,
    ],
    styleUrls: ["./event.component.scss"],
})
export class EventComponent implements OnInit, OnDestroy {

    loading = false;

    eventId: string;
    alertGroup: AlertGroup;
    public event: any = {};

    // An object containing normalized fields from the event when they may
    // differ in the real event based on input configuration.
    normalized: any = {};

    params: any = {};
    flows: any[] = [];

    http: any = null;

    servicesForEvent: any[] = [];

    public commentInputVisible: boolean = false;
    public features: any = {};

    constructor(private route: ActivatedRoute,
                private router: Router,
                private elasticSearch: ElasticSearchService,
                private api: ApiService,
                private eventServices: EventServices,
                private location: Location,
                private eventService: EventService,
                private mousetrap: MousetrapService,
                private ss: EveboxSubscriptionService,
                private configService: ConfigService,
                private toastr: ToastrService) {
    }

    reset() {
        this.eventId = undefined;
        this.alertGroup = undefined;
        this.event = {};
        this.params = {};
        this.flows = [];
        this.normalized = {};
    }

    setup() {
    }

    ngOnInit() {

        if (this.configService.hasFeature(FEATURE_COMMENTS)) {
            this.features["comments"] = true;
        }

        let alertGroup = this.eventService.popAlertGroup();

        this.ss.subscribe(this, this.route.params, (params: any) => {

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

    eventToPcap(what: any) {
        this.api.eventToPcap(what, this.event._source);
    }

    goBack() {
        this.location.back();
    }

    showArchiveButton(): boolean {
        return this.event._source.event_type == "alert" &&
                this.event._source.tags.indexOf("archived") == -1;
    }

    eventType(): string {
        return this.event._source.event_type;
    }

    hasGeoip(): boolean {
        if (this.event._source.geoip &&
                Object.keys(this.event._source.geoip).length > 0) {
            return true;
        }
        return false;
    }

    hasHistory(): boolean {
        try {
            if (this.event._source.evebox.history) {
                return true;
            } else {
                return false;
            }
        }
        catch (err) {
            return false;
        }
    }

    onCommentSubmit(comment: any) {
        if (this.alertGroup) {
            this.api.commentOnAlertGroup(this.alertGroup, comment)
                    .then(() => {
                        this.commentInputVisible = false;
                        this.elasticSearch.getEventById(this.event._id)
                                .then((response: any) => {
                                    this.event = response;
                                });
                    });
        } else {
            this.api.commentOnEvent(this.eventId, comment)
                    .then(() => {
                        this.commentInputVisible = false;
                        this.elasticSearch.getEventById(this.event._id)
                                .then((response: any) => {
                                    this.event = response;
                                });
                    });
        }
    }

    sessionSearch() {
        let q = `alert.signature_id:${this.event._source.alert.signature_id}`;
        q += ` src_ip${this.elasticSearch.keywordSuffix}:"${this.event._source.src_ip}"`;
        q += ` dest_ip${this.elasticSearch.keywordSuffix}:"${this.event._source.dest_ip}"`;

        // This is only for alerts right now.
        q += ` event_type${this.elasticSearch.keywordSuffix}:"alert"`;

        if (this.params && this.params.referer == "/inbox") {
            q += ` -tags:archived`;
        }

        console.log(q);

        this.router.navigate(["/events", {q: q}]);
    }

    archiveEvent() {
        if (this.alertGroup) {
            this.elasticSearch.archiveAlertGroup(this.alertGroup);
            this.alertGroup.event._source.tags.push("archived");
        }
        else {
            this.elasticSearch.archiveEvent(this.event);
        }
        this.location.back();
    }

    escalateEvent() {
        if (this.alertGroup) {
            this.elasticSearch.escalateAlertGroup(this.alertGroup);
            this.alertGroup.escalatedCount = this.alertGroup.count;
        }
        else {
            console.log("Escalating single event.");
            this.elasticSearch.escalateEvent(this.event);
        }
    }

    deEscalateEvent() {
        if (this.alertGroup) {
            this.elasticSearch.removeEscalatedStateFromAlertGroup(this.alertGroup);
            this.alertGroup.escalatedCount = 0;
        }
        else {
            this.elasticSearch.deEscalateEvent(this.event);
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

    findFlow(event: any) {
        if (!event._source.flow_id) {
            console.log("Unable to find flow for event, event does not have a flow id.");
            console.log(event);
            return;
        }

        this.elasticSearch.findFlow({
            flowId: event._source.flow_id,
            proto: event._source.proto.toLowerCase(),
            timestamp: event._source.timestamp,
            srcIp: event._source.src_ip,
            destIp: event._source.dest_ip,
        }).then(response => {
            console.log(response);
            if (response.flows.length > 0) {
                this.flows = response.flows;
            }
            else {
                console.log("No flows found for event.");
            }
        }, error => {
            console.log("Failed to find flows for event:");
            console.log(error);
        });
    }

    refresh() {
        this.loading = true;

        this.elasticSearch.getEventById(this.eventId)
                .then((event: any) => {
                    let http = null;

                    //this.event = event;
                    if (event._source.event_type != "flow") {
                        this.findFlow(event);
                    }

                    // Break out some fields of the event.
                    if (event._source.http) {
                        http = {};
                        let _http = event._source.http;
                        for (let key in _http) {
                            switch (key) {
                                case "http_response_body_printable":
                                case "http_request_body_printable":
                                case "http_response_body":
                                case "http_request_body":
                                    break;
                                default:
                                    http[key] = _http[key];
                                    break;
                            }
                        }
                    }

                    // If the Suricata provided rule doesn't exist, check for
                    // an EveBox added one and put it where the Suricata one
                    // would be found.
                    if (event._source.alert) {
                        if (!event._source.alert.rule && event._source.rule) {
                            event._source.alert.rule = event._source.rule;
                        }
                    }

                    // Normalize the sensor name.
                    // - Suricata put the name in the "host" field.
                    // - Filebeat will overwrite the "host" field with its own "host" object,
                    //       losing the Suricata sensor name. So use the Filebeat provided
                    //       hostname instead.
                    if (event._source.host) {
                        if (event._source.host.name) {
                            this.normalized.sensor_name = event._source.host.name;
                            this.normalized.sensor_name_key = "host.name";
                        } else if (typeof (event._source.host) === "string") {
                            this.normalized.sensor_name = event._source.host;
                            this.normalized.sensor_name_key = "host";
                        }
                    }

                    this.servicesForEvent = this.eventServices.getServicesForEvent(this.event);
                    this.event = event;
                    this.http = http;
                    this.loading = false;
                })
                .catch((error: any) => {
                    console.log("error:");
                    console.log(error);
                    this.notifyError(error);
                    this.loading = false;
                });
    }

    notifyError(error: any) {
        try {
            this.toastr.error(error.error.message);
            return;
        }
        catch (e) {
        }

        this.toastr.error("Unhandled error: " + JSON.stringify(error));
    }
}
