// Copyright (C) 2014-2020 Jason Ish
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

import { Injectable } from "@angular/core";
import { Router } from "@angular/router";
import { ConfigService } from "./config.service";
import { Observable } from "rxjs";
import { ClientService, LoginResponse } from "./client.service";
import { catchError, finalize, map } from "rxjs/operators";
import { HttpClient, HttpHeaders, HttpParams } from "@angular/common/http";
import { throwError } from "rxjs/internal/observable/throwError";

declare var localStorage: any;

const SESSION_HEADER = "x-evebox-session-id";

/**
 * The API service exposes the server side API to the rest of the server,
 * and acts as the "client" to the server.
 */
@Injectable()
export class ApiService {

    private authenticated = false;

    constructor(private httpClient: HttpClient,
                public client: ClientService,
                private router: Router,
                private configService: ConfigService) {
        this.client._sessionId = localStorage.sessionId;
    }

    isAuthenticated(): boolean {
        return this.authenticated;
    }

    setSessionId(sessionId: string | null): void {
        this.client.setSessionId(sessionId);
    }

    checkVersion(response: any): void {
        this.client.checkVersion(response);
    }

    applySessionHeader(options: any): void {
        if (this.client._sessionId) {
            const headers = options.headers || new Headers();
            headers.append(SESSION_HEADER, this.client._sessionId);
            options.headers = headers;
        }
    }

    setSessionHeader(headers: HttpHeaders): HttpHeaders {
        if (this.client._sessionId) {
            return headers.set(SESSION_HEADER, this.client._sessionId);
        }
        return headers;
    }

    setAuthenticated(authenticated: boolean): void {
        this.authenticated = authenticated;
        this.client.setAuthenticated(authenticated);
        if (!authenticated) {
            this.setSessionId(null);
            this.router.navigate(["/login"]).then(() => {
            });
        }
    }

    private handle401(): void {
        this.setAuthenticated(false);
    }

    /**
     * Low level options request, just fixup the URL.
     */
    _options(path: string): Observable<any> {
        return this.httpClient.options(this.client.buildUrl(path));
    }

    doRequest(method: string, path: string, options: any = {}): Observable<any> {
        const headers = options.headers || new HttpHeaders();
        options.headers = this.setSessionHeader(headers);
        options.observe = "response";
        return this.httpClient.request<any>(method, path, options)
            .pipe(map((response: any) => {
                this.client.updateSessionId(response);
                this.checkVersion(response);
                return response.body;
            }), catchError((error) => {
                if (error.error instanceof ErrorEvent) {
                    // Client side or network error.
                } else {
                    if (error.status === 401) {
                        this.handle401();
                    }
                }
                return throwError(error);
            }));
    }

    post(path: string, body: any, options: any = {}): Promise<any> {
        options.body = body;
        return this.doRequest("POST", path, options).toPromise();
    }

    updateConfig(): Promise<any> {
        return this.client.get("api/1/config").toPromise()
            .then((config) => {
                this.configService.setConfig(config);
                return config;
            });
    }

    checkAuth(): Promise<true | false> {
        return this.updateConfig()
            .then(() => {
                this.setAuthenticated(true);
                return true;
            })
            .catch((error) => {
                console.log("updateConfig failed:");
                console.log(error);
                return false;
            });
    }

    login(username: string = "", password: string = ""): Promise<boolean> {
        return this.client.login(username, password).toPromise()
            .then((response: LoginResponse) => {
                this.setSessionId(response.session_id);
                this.setAuthenticated(true);
                return this.updateConfig()
                    .then(() => {
                        return true;
                    });
            });
    }

    logout(): Promise<any> {
        return this.client.logout().pipe(
            finalize(() => {
                this.setAuthenticated(false);
            })
        ).toPromise();
    }

    getWithParams(path: string, params = {}): Promise<any> {

        const qsb: any = [];

        for (const param of Object.keys(params)) {
            qsb.push(`${param}=${params[param]}`);
        }

        return this.client.get(`${path}?${qsb.join("&")}`).toPromise();
    }

    getVersion(): Promise<any> {
        return this.client.get("api/1/version").toPromise();
    }

    eventToPcap(what: any, event: any): void {
        // Set a cook with the session key to expire in 60 seconds from now.
        const expires = new Date(new Date().getTime() + 60000);
        const cookie = `${SESSION_HEADER}=${this.client._sessionId}; expires=${expires.toUTCString()}`;
        console.log("Setting cookie: " + cookie);
        document.cookie = cookie;

        const form = document.createElement("form") as HTMLFormElement;
        form.setAttribute("method", "post");
        form.setAttribute("action", "api/1/eve2pcap");

        const whatField = document.createElement("input") as HTMLElement;
        whatField.setAttribute("type", "hidden");
        whatField.setAttribute("name", "what");
        whatField.setAttribute("value", what);
        form.appendChild(whatField);

        const eventField = document.createElement("input") as HTMLElement;
        eventField.setAttribute("type", "hidden");
        eventField.setAttribute("name", "event");
        eventField.setAttribute("value", JSON.stringify(event));
        form.appendChild(eventField);

        document.body.appendChild(form);
        form.submit();
    }

    reportHistogram(options: ReportHistogramOptions = {}): Promise<any> {
        const query: any = [];

        if (options.timeRange && options.timeRange > 0) {
            query.push(`timeRange=${options.timeRange}s`);
        }

        if (options.interval) {
            query.push(`interval=${options.interval}`);
        }

        if (options.addressFilter) {
            query.push(`addressFilter=${options.addressFilter}`);
        }

        if (options.queryString) {
            query.push(`queryString=${options.queryString}`);
        }

        if (options.sensorFilter) {
            query.push(`sensorFilter=${options.sensorFilter}`);
        }

        if (options.dnsType) {
            query.push(`dnsType=${options.dnsType}`);
        }

        if (options.eventType) {
            query.push(`eventType=${options.eventType}`);
        }

        return this.client.get(`api/1/report/histogram?${query.join("&")}`).toPromise();
    }

    reportAgg(agg: string, options: ReportAggOptions = {}): Promise<any> {
        let params = new HttpParams().append("agg", agg);

        for (const key of Object.keys(options)) {
            switch (key) {
                case "timeRange":
                    params = params.append("timeRange", `${options[key]}s`);
                    break;
                default:
                    params = params.append(key, options[key]);
                    break;
            }
        }

        return this.client.get("api/1/report/agg", params).toPromise();
    }

    /**
     * Find events - all events, not just alerts.
     */
    eventQuery(options: EventQueryOptions = {}): Observable<any> {

        let params = new HttpParams();

        if (options.queryString) {
            params = params.append("query_string", options.queryString);
        }

        if (options.maxTs) {
            params = params.append("max_ts", options.maxTs);
        }

        if (options.minTs) {
            params = params.append("min_ts", options.minTs);
        }

        if (options.eventType && options.eventType !== "all") {
            params = params.append("event_type", options.eventType);
        }

        if (options.sortOrder) {
            params = params.append("order", options.sortOrder);
        }

        if (options.sortBy) {
            params = params.append("sort_by", options.sortBy);
        }

        if (options.size) {
            params = params.append("size", options.size.toString());
        }

        if (options.timeRange) {
            params = params.append("time_range", `${options.timeRange}s`);
        }

        return this.client.get("api/1/event-query", params);
    }

    flowHistogram(args: any = {}): any {
        let params = new HttpParams();

        const subAggs = [];
        if (args.appProto) {
            subAggs.push("app_proto");
        }
        if (subAggs.length > 0) {
            params = params.append("sub_aggs", subAggs.join(","));
        }

        if (args.timeRange) {
            params = params.append("time_range", args.timeRange);
        }

        if (args.queryString) {
            params = params.append("query_string", args.queryString);
        }

        if (args.interval) {
            params = params.append("interval", args.interval);
        }

        return this.client.get("api/1/flow/histogram", params);
    }

    commentOnEvent(eventId: string, comment: string): Promise<any> {
        console.log(`Commenting on event ${eventId}.`);
        return this.post(`api/1/event/${eventId}/comment`, {
            event_id: eventId,
            comment,
        });
    }

    commentOnAlertGroup(alertGroup: any, comment: string): Promise<any> {
        console.log(`Commenting on alert group:`);
        console.log(alertGroup);

        const request = {
            signature_id: alertGroup.event._source.alert.signature_id,
            src_ip: alertGroup.event._source.src_ip,
            dest_ip: alertGroup.event._source.dest_ip,
            min_timestamp: alertGroup.minTs,
            max_timestamp: alertGroup.maxTs,
        };

        return this.post(`api/1/alert-group/comment`, {
            alert_group: request,
            comment: comment,
        });
    }

    alertQuery(options: {
        queryString?: string;
        mustHaveTags?: any[];
        mustNotHaveTags?: any[];
        timeRange?: string;
    }): Observable<any> {
        let params = new HttpParams();
        const tags: string[] = [];

        if (options.mustHaveTags) {
            options.mustHaveTags.forEach((tag: string) => {
                tags.push(tag);
            });
        }

        if (options.mustNotHaveTags) {
            options.mustNotHaveTags.forEach((tag: string) => {
                tags.push(`-${tag}`);
            });
        }

        params = params.append("tags", tags.join(","));
        params = params.append("time_range", options.timeRange);
        params = params.append("query_string", options.queryString);

        return this.client.get("api/1/alerts", params);
    }


}

export interface ReportHistogramOptions {
    timeRange?: number;
    interval?: string;
    addressFilter?: string;
    queryString?: string;
    sensorFilter?: string;
    eventType?: string;
    dnsType?: string;
}

// Options for an aggregation report.
export interface ReportAggOptions {
    size?: number;
    queryString?: string;
    timeRange?: number;

    // Event type.
    eventType?: string;

    // Subtype info.
    dnsType?: string;

}

export interface EventQueryOptions {
    queryString?: string;
    maxTs?: string;
    minTs?: string;
    eventType?: string;
    sortOrder?: string;
    sortBy?: string;
    size?: number;
    timeRange?: number;
}
