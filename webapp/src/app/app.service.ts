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

import {EventEmitter, Injectable} from "@angular/core";
import {ActivatedRoute, Router} from "@angular/router";
import {MousetrapService} from "./mousetrap.service";
import {NgbModal} from "@ng-bootstrap/ng-bootstrap";
import {HelpComponent} from "./help/help.component";

export let FEATURE_REPORTING = "reporting";
export let FEATURE_COMMENTS = "comments";

export enum AppEventCode {
    TIME_RANGE_CHANGED,
    IDLE,
}

export interface AppEvent {
    event: AppEventCode;
    data?: any;
}

@Injectable()
export class AppService {

    private eventEmitter: EventEmitter<AppEvent> = new EventEmitter<AppEvent>();

    timeRangeDisabled = false;

    private lastRouteEvent: number = new Date().getTime() / 1000;

    constructor(private router: Router,
                private route: ActivatedRoute,
                private mousetrap: MousetrapService,
                private modalService: NgbModal) {

        mousetrap.bindAny(this, () => {
            this.resetIdleTime();
        });

        // Setup idle check interval.
        setInterval(() => this.dispatchIdleEvent(), 1000);
    }

    dispatchIdleEvent() {
        let now = new Date().getTime() / 1000;
        let idleTime = now - this.lastRouteEvent;
        this.dispatch({event: AppEventCode.IDLE, data: idleTime});
    }

    resetIdleTime() {
        this.lastRouteEvent = new Date().getTime() / 1000;
    }

    isTimeRangeDisabled() {
        return this.timeRangeDisabled;
    }

    enableTimeRange() {
        console.log("Enabling time range.");
        this.timeRangeDisabled = false;
    }

    disableTimeRange() {
        console.log("Disabling time range.");
        this.timeRangeDisabled = true;
    }

    subscribe(handler: any) {
        return this.eventEmitter.subscribe(handler);
    }

    dispatch(event: AppEvent) {
        this.eventEmitter.emit(event);
    }

    getRoute() {
        // First get the name of the first part of the path without query
        // parameters, but after the first /.
        let route = this.router.url.substring(1).split(/[;\?\/]/)[0];

        // Return the route with a leading / as that is what is expected right
        // now.
        return "/" + route;
    }

    updateParams(activatedRoute: ActivatedRoute, params: any = {}, queryParams: any = {}) {

        let newParams = JSON.parse(JSON.stringify(activatedRoute.snapshot.params));
        let newQueryParams = JSON.parse(JSON.stringify(activatedRoute.snapshot.queryParams));

        Object.keys(params).forEach((key: any) => {
            let value = params[key];
            if (value == undefined || value == null) {
                delete(newParams[key]);
            }
            else {
                newParams[key] = value;
            }
        });

        Object.keys(queryParams).forEach((key: any) => {
            let value = queryParams[key];
            if (value == undefined || value == null) {
                delete(newQueryParams[key]);
            }
            else {
                newQueryParams[key] = value;
            }
        });

        let path = this.router.url.split( /[;\?]/)[0];
        this.router.navigate([path, newParams], {
            queryParams: newQueryParams
        });
    }

    showHelp() {
        this.modalService.open(HelpComponent, { size: "lg"});
    }

}