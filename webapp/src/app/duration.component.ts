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

import { Component, Input, OnInit, OnDestroy, NgZone } from "@angular/core";
//import moment = require("moment");
import * as moment from "moment";

@Component({
    selector: "evebox-duration",
    template: "{{duration}} ago",
})
export class EveboxDurationComponent implements OnInit, OnDestroy {
    @Input() private timestamp: any;
    duration: any;
    interval: any = null;

    constructor(private ngZone: NgZone) {}

    refresh() {
        let then = moment(this.timestamp);
        let now = moment();
        let diff = then.diff(now);
        let duration = moment.duration(diff);
        //noinspection TypeScriptUnresolvedFunction
        this.duration = duration.humanize();
    }

    ngOnInit() {
        this.refresh();

        this.interval = window.setInterval(() => {
            this.ngZone.run(() => {
                this.refresh();
            });
        }, 60000);
    }

    ngOnDestroy(): any {
        if (this.interval != null) {
            clearInterval(this.interval);
        }
    }
}
