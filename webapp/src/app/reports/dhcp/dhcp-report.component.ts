// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

import {HttpParams} from "@angular/common/http";
import {Component, OnDestroy, OnInit} from "@angular/core";
import {AppEventCode, AppService} from "src/app/app.service";
import {ClientService} from "src/app/client.service";
import {TopNavService} from "src/app/topnav.service";
import * as moment from "moment";

@Component({
    selector: "app-dhcp",
    templateUrl: "./dhcp-report.component.html",
    styleUrls: ["./dhcp-report.component.scss"]
})
export class DhcpReportComponent implements OnInit, OnDestroy {

    private subs = [];

    acks: any[] = [];
    requests: any[] = [];
    report: any[] = [];

    constructor(private appService: AppService, private client: ClientService, private topNavService: TopNavService) {
    }

    ngOnInit(): void {
        this.subs.push(this.appService.subscribe((event: any) => {
            if (event.event === AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        }));

        this.refresh();
    }

    ngOnDestroy(): void {
        this.subs.forEach((s) => s.unsubscribe());
    }

    refresh(): void {
        const range = this.topNavService.getTimeRangeAsSeconds();
        let params = new HttpParams();
        params = params.append("time_range", `${range}s`);

        const now = moment().unix();

        this.client.get("/api/1/report/dhcp/request", params).subscribe((requests) => {
            this.client.get("/api/1/report/dhcp/ack", params).subscribe((acks) => {
                const merged: any = {};

                this.requests = requests.data;
                this.acks = acks.data;
                for (const request of requests.data) {
                    merged[request.dhcp.client_mac] = {
                        timestamp: request.timestamp,
                        client_mac: request.dhcp.client_mac,
                        hostname: request.dhcp.hostname,
                    };
                }
                for (const ack of acks.data) {
                    let record = merged[ack.dhcp.client_mac];
                    if (!record) {
                        // This is most likely due to DHCP extended logs not being enabled.
                        record = {
                            timestamp: ack.timestamp,
                            client_mac: ack.dhcp.client_mac,
                        }
                        merged[record.client_mac] = record;
                    }
                    record.assigned_ip = ack.dhcp.assigned_ip;
                    if (ack.dhcp.hostname) {
                        record.hostname = ack.dhcp.hostname;
                    }

                    const ackTs = moment(ack.timestamp).unix();
                    const lease_time = ack.dhcp.lease_time;
                    const active = ackTs + lease_time > now;
                    record.active = active;
                    record.lease_time = lease_time;
                }

                this.report = Object.keys(merged).sort((a, b) => {
                    return moment(merged[b].timestamp).unix() - moment(merged[a].timestamp).unix();
                }).map((z) => {
                    return merged[z];
                });

            });
        });
    }

    quote(val: string): string {
        return `"${val}"`;
    }
}
