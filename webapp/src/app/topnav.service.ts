// Copyright (C) 2016-2021 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

import { Injectable } from "@angular/core";
import * as moment from "moment";
import { ConfigService } from "./config.service";

declare var localStorage: any;

@Injectable()
export class TopNavService {
    timeRange = "24h";

    constructor(private config: ConfigService) {
        const forceDefaultTimeRange = config.getDefault("force_time_range");
        const defaultTimeRange = config.getDefault("time_range");
        const localTimeRange = localStorage.timeRange;

        if (defaultTimeRange && (forceDefaultTimeRange || !localTimeRange)) {
            if (defaultTimeRange === "all") {
                this.timeRange = "";
            } else {
                this.timeRange = defaultTimeRange;
            }
        } else if (localTimeRange) {
            this.timeRange = localTimeRange;
        }
    }

    setTimeRange(timeRange: string): void {
        this.timeRange = timeRange;
        localStorage.timeRange = timeRange;
    }

    /**
     * Get the time range in seconds.
     */
    getTimeRangeAsSeconds(): number {
        if (this.timeRange === "") {
            // Everything...
            return 0;
        }
        const parts: any[] = this.timeRange.match(/(\d+)(\w+)/) as any[];
        const value: number = parseInt(parts[1], 10);
        const unit: string = parts[2];
        return moment.duration(value, unit as any).asSeconds();
    }
}
