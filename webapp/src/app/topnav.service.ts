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

import {Injectable} from "@angular/core";
import * as moment from "moment";
import {ConfigService} from "./config.service";

declare var localStorage: any;

@Injectable()
export class TopNavService {

    timeRange = "24h";

    constructor(private config: ConfigService) {
        let forceDefaultTimeRange = config.getDefault("force_time_range");
        let defaultTimeRange = config.getDefault("time_range");
        let localTimeRange = localStorage.timeRange;

        if (defaultTimeRange && (forceDefaultTimeRange || !localTimeRange)) {
            if (defaultTimeRange == "all") {
                this.timeRange = "";
            } else {
                this.timeRange = defaultTimeRange;
            }
        } else if (localTimeRange) {
            this.timeRange = localTimeRange;
        }
    }

    setTimeRange(timeRange: string) {
        this.timeRange = timeRange;
        localStorage.timeRange = timeRange;
    }

    /**
     * Get the time range in seconds.
     */
    getTimeRangeAsSeconds(): any {
        if (this.timeRange == "") {
            // Everything...
            return 0;
        }
        let parts: any[] = <any[]>this.timeRange.match(/(\d+)(\w+)/);
        let value: number = parseInt(parts[1]);
        let unit: string = parts[2];
        return moment.duration(value, <any>unit).asSeconds();
    }
}