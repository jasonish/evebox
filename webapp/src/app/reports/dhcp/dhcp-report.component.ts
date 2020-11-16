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
import {AfterViewInit, Component, OnDestroy, OnInit} from "@angular/core";
import {AppEventCode, AppService} from "src/app/app.service";
import {ClientService} from "src/app/client.service";
import {TopNavService} from "src/app/topnav.service";
import * as moment from "moment";
import {ActivatedRoute, Params} from "@angular/router";
import {Observable} from "rxjs";
import {finalize} from "rxjs/operators";
import {animate, state, style, transition, trigger} from "@angular/animations";
import {spinningLoaderAnimation} from "../../animations";

declare var $: any;

@Component({
    selector: "app-dhcp",
    templateUrl: "./dhcp-report.component.html",
    styleUrls: ["./dhcp-report.component.scss"],
    animations: [spinningLoaderAnimation],
})
export class DhcpReportComponent implements OnInit, OnDestroy, AfterViewInit {

    private subs = [];

    acks: any[] = [];
    requests: any[] = [];
    report: any[] = [];
    servers: any[] = [];
    ip: any[] = [];
    mac: any[] = [];

    haveSensorName = false;
    queryString = "";

    loading = 0;

    constructor(private appService: AppService, private client: ClientService, private topNavService: TopNavService,
                private route: ActivatedRoute) {
    }

    ngOnInit(): void {
        this.subs.push(this.appService.subscribe((event: any) => {
            if (event.event === AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        }));

        this.route.queryParams.subscribe((params: Params) => {
            console.log("Got new route parameters...");
            this.queryString = params.q || "";
            this.refresh();
        });

        this.refresh();
    }

    ngOnDestroy(): void {
        this.subs.forEach((s) => s.unsubscribe());
    }

    ngAfterViewInit(): void {
        // tslint:disable-next-line:quotemark
        $('[data-toggle="tooltip"]').tooltip();
    }

    refresh(): void {
        let haveSensorName = false;
        let params = new HttpParams();
        const range = this.topNavService.getTimeRangeAsSeconds();
        if (range > 0) {
            params = params.append("time_range", `${range}s`);
        }
        if (this.queryString && this.queryString !== "") {
            params = params.append("query_string", this.queryString);
        }

        const now = moment().unix();

        this.load(this.client.get("/api/1/report/dhcp/request", params)).subscribe((requests) => {
            this.load(this.client.get("/api/1/report/dhcp/ack", params)).subscribe((acks) => {
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
                    if (ack.host) {
                        haveSensorName = true;
                    }
                    let record = merged[ack.dhcp.client_mac];
                    if (!record) {
                        // This is most likely due to DHCP extended logs not being enabled.
                        record = {
                            timestamp: ack.timestamp,
                            client_mac: ack.dhcp.client_mac,
                        };
                        merged[record.client_mac] = record;
                    }
                    record.assigned_ip = ack.dhcp.assigned_ip;
                    record.sensor = ack.host;
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

                this.haveSensorName = haveSensorName;

            });
        });

        this.load(this.client.get("/api/1/report/dhcp/servers", params)).subscribe((response) => {
            this.servers = response.data;
        });

        this.load(this.client.get("/api/1/report/dhcp/mac", params)).subscribe((response) => {
            this.mac = response.data.filter((entry) => entry.addrs.length > 1).map((entry) => entry.mac);
        });

        this.load(this.client.get("/api/1/report/dhcp/ip", params)).subscribe((response) => {
            this.ip = response.data.filter((entry) => entry.macs.length > 1).map((entry) => entry.ip);
        });

    }

    private load(o: Observable<any>) {
        this.loading += 1;
        return o.pipe(finalize(() => {
            if (this.loading > 0) {
                this.loading -= 1;
            }
        }));
    }

    quote(val: string): string {
        return `"${val}"`;
    }
}
