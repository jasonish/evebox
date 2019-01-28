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
import {ActivatedRoute} from "@angular/router";
import {ElasticSearchService} from "../elasticsearch.service";
import {MousetrapService} from "../mousetrap.service";
import {AppService} from "../app.service";
import {ToastrService} from "../toastr.service";
import {EveboxSubscriptionService} from "../subscription.service";
import {loadingAnimation} from "../animations";
import {ApiService} from "../api.service";
import {finalize} from "rxjs/operators";
import {EVENT_TYPES} from '../shared/eventtypes';

@Component({
    templateUrl: "./events.component.html",
    animations: [
        loadingAnimation,
    ]
})
export class EventsComponent implements OnInit, OnDestroy {

    model: any = {
        newestTimestamp: "",
        oldestTimestamp: "",
        events: [],
    };

    loading = false;

    queryString = "";

    eventTypes = EVENT_TYPES;

    // Error to be display if set.
    error: string = null;

    eventTypeFilter: any = this.eventTypes[0];

    timeStart: string;
    timeEnd: string;
    private order: string;

    constructor(private route: ActivatedRoute,
                private elasticsearch: ElasticSearchService,
                private mousetrap: MousetrapService,
                private appService: AppService,
                private toastr: ToastrService,
                private api: ApiService,
                private ss: EveboxSubscriptionService) {
    }

    ngOnInit(): any {
        this.ss.subscribe(this, this.route.params, (params: any) => {
            let qp: any = this.route.snapshot.queryParams;

            this.queryString = params.q || qp.q || "";
            this.timeStart = params.timeStart || qp.timeStart;
            this.timeEnd = params.timeEnd || qp.timeEnd;

            if (params.eventType) {
                this.setEventTypeFilterByEventType(params.eventType);
            }

            this.order = params.order;
            this.refresh();
        });

        // Use setTimeout to prevent ExpressionChangedAfterItHasBeenCheckedError.
        setTimeout(() => {
            this.appService.disableTimeRange();
        }, 0);

        this.mousetrap.bind(this, "/", () => this.focusFilterInput());
        this.mousetrap.bind(this, "r", () => this.refresh());
    }

    setEventTypeFilterByEventType(eventType: string) {
        for (let et of this.eventTypes) {
            if (et.eventType == eventType) {
                this.eventTypeFilter = et;
                break;
            }
        }
    }

    setEventTypeFilter(type: any) {
        this.eventTypeFilter = type;
        this.appService.updateParams(this.route, {eventType: this.eventTypeFilter.eventType});
    }

    ngOnDestroy() {
        this.mousetrap.unbind(this);
        this.ss.unsubscribe(this);
    }

    focusFilterInput() {
        document.getElementById("filter-input").focus();
    }

    submitFilter() {
        document.getElementById("filter-input").blur();
        this.appService.updateParams(this.route, {
            q: this.queryString
        });
    }

    clearFilter() {
        this.queryString = "";
        this.submitFilter();
    }

    gotoNewest() {
        this.appService.updateParams(this.route, {
            timeStart: undefined,
            timeEnd: undefined,
            order: "desc",
        });
    }

    gotoNewer() {
        this.appService.updateParams(this.route, {
            timeEnd: undefined,
            timeStart: this.model.newestTimestamp,
            order: "asc",
        });
    }

    gotoOlder() {
        console.log(`gotoOlder: timeEnd=${this.model.oldestTimestamp}`);
        this.appService.updateParams(this.route, {
            timeEnd: this.model.oldestTimestamp,
            timeStart: undefined,
            order: "desc",
        });
    }

    gotoOldest() {
        this.appService.updateParams(this.route, {
            timeEnd: undefined,
            timeStart: undefined,
            order: "asc",
        });
    }

    hasEvents(): boolean {
        try {
            return this.model.events.length > 0;
        } catch (err) {
            return false;
        }
    }

    refresh() {
        this.clearError();
        this.model.events = [];
        this.loading = true;

        this.api.eventQuery({
            queryString: this.queryString,
            maxTs: this.timeEnd,
            minTs: this.timeStart,
            eventType: this.eventTypeFilter.eventType,
            sortOrder: this.order,
        }).pipe(finalize(() => {
            this.loading = false;
        })).subscribe((response) => {
            let events = response.data;

            // If the sortOrder is "asc", reverse to put back into descending sortOrder.
            if (this.order == "asc") {
                events = events.reverse();
            }

            if (events.length > 0) {
                this.model.newestTimestamp = events[0]._source["@timestamp"];
                this.model.oldestTimestamp = events[events.length - 1]._source["@timestamp"];

                console.log(`Newest event: ${this.model.newestTimestamp}`);
                console.log(`Oldest event: ${this.model.oldestTimestamp}`);
            }
            this.model.events = events;
        }, (error) => {
            this.setError(error);
        });
    }

    private setError(error: string) {
        this.error = error;
    }

    private clearError() {
        this.error = null;
    }

}
