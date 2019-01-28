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

import {Injectable} from "@angular/core";
import {ToastrService} from "./toastr.service";
import {GITREV} from "../environments/gitrev";
import {Router} from "@angular/router";
import {ConfigService} from "./config.service";
import {Observable} from "rxjs";

import {ClientService, LoginResponse} from "./client.service";
import {catchError, finalize, map} from "rxjs/operators";
import {HttpClient, HttpHeaders, HttpParams} from "@angular/common/http";
import {throwError} from 'rxjs/internal/observable/throwError';

declare var localStorage: any;

const SESSION_HEADER = "x-evebox-session-id";

/**
 * The API service exposes the server side API to the rest of the server,
 * and acts as the "client" to the server.
 */
@Injectable()
export class ApiService {

    private authenticated = false;

    private versionWarned = false;

    private sessionId: string;

    constructor(private httpClient: HttpClient,
                private client: ClientService,
                private toastr: ToastrService,
                private router: Router,
                private configService: ConfigService) {
        this.sessionId = localStorage.sessionId;
    }

    isAuthenticated(): boolean {
        return this.authenticated;
    }

    setSessionId(sessionId: string | null) {
        this.sessionId = sessionId;
        localStorage.sessionId = sessionId;
        this.client.setSessionId(sessionId);
    }

    checkVersion(response: any) {
        if (this.versionWarned) {
            return;
        }
        let webappRev: string = GITREV;
        let serverRev: string = response.headers.get("x-evebox-git-revision");
        if (webappRev !== serverRev) {
            console.log(`Server version: ${serverRev}; webapp version: ${webappRev}`);
            this.toastr.warning(
                    `The EveBox server has been updated.
             Please reload</a>.
             <br><a href="javascript:window.location.reload()"
             class="btn btn-primary btn-block">Reload Now</a>`, {
                        closeButton: true,
                        timeOut: 0,
                        extendedTimeOut: 0,
                    });
            this.versionWarned = true;
        }
    }

    applySessionHeader(options: any) {
        if (this.sessionId) {
            let headers = options.headers || new Headers();
            headers.append(SESSION_HEADER, this.sessionId);
            options.headers = headers;
        }
    }

    setSessionHeader(headers: HttpHeaders): HttpHeaders {
        if (this.sessionId) {
            return headers.set(SESSION_HEADER, this.sessionId);
        }
        return headers;
    }

    setAuthenticated(authenticated: boolean) {
        this.authenticated = authenticated;
        this.client.setAuthenticated(authenticated);
        if (!authenticated) {
            this.setSessionId(null);
            this.router.navigate(["/login"]);
        }
    }

    private updateSessionId(response: any) {
        let sessionId = response.headers.get(SESSION_HEADER);
        if (sessionId && sessionId != this.sessionId) {
            console.log("Updating session ID from response header.");
            this.setSessionId(sessionId);
        }
    }

    private handle401() {
        this.setAuthenticated(false);
    }

    /**
     * Low level options request, just fixup the URL.
     */
    _options(path: string): Observable<any> {
        return this.httpClient.options(this.client.buildUrl(path))
    }

    doRequest(method: string, path: string, options: any = {}): Observable<any> {
        let headers = options.headers || new HttpHeaders();
        options.headers = this.setSessionHeader(headers);
        options.observe = "response";
        return this.httpClient.request<any>(method, path, options)
                .pipe(map((response: any) => {
                    this.updateSessionId(response);
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

    get(path: string, options: any = {}): Promise<any> {
        return this.doRequest("GET", path, options).toPromise();
    }

    updateConfig(): Promise<any> {
        return this.get("api/1/config")
                .then((config) => {
                    this.configService.setConfig(config);
                    return config;
                });
    }

    checkAuth() {
        return this.updateConfig()
                .then(config => {
                    this.setAuthenticated(true);
                    this.client.setAuthenticated(true);
                    return true;
                })
                .catch(() => false);
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

    logout() : Promise<any> {
        return this.client.logout().pipe(
                finalize(() => {
                    this.setAuthenticated(false);
                })
        ).toPromise();
    }

    getWithParams(path: string, params = {}): Promise<any> {

        let qsb: any = [];

        for (let param in params) {
            qsb.push(`${param}=${params[param]}`);
        }

        return this.get(`${path}?${qsb.join("&")}`);
    }

    getVersion() {
        return this.get("api/1/version");
    }

    eventToPcap(what: any, event: any) {
        // Set a cook with the session key to expire in 60 seconds from now.
        const expires = new Date(new Date().getTime() + 60000);
        const cookie = `${SESSION_HEADER}=${this.sessionId}; expires=${expires.toUTCString()}`;
        console.log("Setting cookie: " + cookie);
        document.cookie = cookie;

        let form = <HTMLFormElement>document.createElement("form");
        form.setAttribute("method", "post");
        form.setAttribute("action", "api/1/eve2pcap");

        let whatField = <HTMLElement>document.createElement("input");
        whatField.setAttribute("type", "hidden");
        whatField.setAttribute("name", "what");
        whatField.setAttribute("value", what);
        form.appendChild(whatField);

        let eventField = <HTMLElement>document.createElement("input");
        eventField.setAttribute("type", "hidden");
        eventField.setAttribute("name", "event");
        eventField.setAttribute("value", JSON.stringify(event));
        form.appendChild(eventField);

        document.body.appendChild(form);
        form.submit();
    }

    reportHistogram(options: ReportHistogramOptions = {}) {
        let query: any = [];

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

        return this.get(`api/1/report/histogram?${query.join("&")}`);
    }

    reportAgg(agg: string, options: ReportAggOptions = {}) {
        let params = new HttpParams().append("agg", agg);

        for (let option in options) {
            switch (option) {
                case "timeRange":
                    params = params.append("timeRange", `${options[option]}s`);
                    break;
                default:
                    params = params.append(option, options[option]);
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

        if (options.eventType && options.eventType != "all") {
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

        let subAggs = [];
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

    commentOnEvent(eventId: string, comment: string) {
        console.log(`Commenting on event ${eventId}.`);
        return this.post(`api/1/event/${eventId}/comment`, {
            "event_id": eventId,
            "comment": comment,
        });
    }

    commentOnAlertGroup(alertGroup: any, comment: string) {
        console.log(`Commenting on alert group:`);
        console.log(alertGroup);

        let request = {
            signature_id: alertGroup.event._source.alert.signature_id,
            src_ip: alertGroup.event._source.src_ip,
            dest_ip: alertGroup.event._source.dest_ip,
            min_timestamp: alertGroup.minTs,
            max_timestamp: alertGroup.maxTs,
        };

        return this.post(`api/1/alert-group/comment`, {
            "alert_group": request,
            "comment": comment,
        });
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
