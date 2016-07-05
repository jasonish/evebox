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

import {Injectable, EventEmitter} from "@angular/core";
import {Router, Params} from "@angular/router";

export enum AppEventCode {
    SHOW_HELP = 0,
    TIME_RANGE_CHANGED,
}

export interface AppEvent {
    event:AppEventCode,
    data?:any
}

@Injectable()
export class AppService {

    private eventEmitter:EventEmitter<AppEvent> = new EventEmitter<AppEvent>();

    private timeRangeDisabled:boolean = false;

    constructor(private router:Router) {
    }

    isTimeRangeDisabled() {
        return this.timeRangeDisabled;
    }

    enableTimeRange() {
        this.timeRangeDisabled = false;
    }

    disableTimeRange() {
        this.timeRangeDisabled = true;
    }

    subscribe(handler:any) {
        return this.eventEmitter.subscribe(handler);
    }

    dispatch(event:AppEvent) {
        this.eventEmitter.emit(event);
    }

    getRoute() {
        return this.router.url.split("?")[0];
    }

    updateQueryParameters(params:any) {
        let queryParams:Params = this.router.routerState.snapshot.queryParams;
        Object.keys(params).forEach((key:any) => {
            let value = params[key];
            if (value) {
                queryParams[key] = params[key];
            }
            else {
                delete queryParams[key];
            }
        });
        this.router.navigate([this.getRoute()], {queryParams: queryParams});
    }
}