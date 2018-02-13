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
import {Headers, Http, RequestOptionsArgs, Response} from "@angular/http";
import {ToastrService} from "./toastr.service";
import {GITREV} from "../environments/gitrev";
import {Router} from "@angular/router";
import {ConfigService} from "./config.service";
import {Observable} from "rxjs/Observable";

import "rxjs/add/observable/throw";
import "rxjs/add/operator/catch";
import "rxjs/add/operator/map";
import {BehaviorSubject} from "rxjs/BehaviorSubject";
import {ClientService, LoginResponse} from "./client.service";
import {finalize} from "rxjs/operators";
import {HttpClient, HttpParams} from "@angular/common/http";

declare var localStorage: any;

/**
 * The API service exposes the server side API to the rest of the server,
 * and acts as the "client" to the server.
 */
@Injectable()
export class ApiService {

    private authenticated = false;

    private versionWarned = false;

    private sessionId: string;

    constructor(private http: Http,
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

    checkVersion(response: Response) {
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

    applySessionHeader(options: RequestOptionsArgs) {
        if (this.sessionId) {
            let headers = options.headers || new Headers();
            headers.append("x-evebox-session-id", this.sessionId);
            options.headers = headers;
        }
    }

    setAuthenticated(authenticated: boolean) {
        this.authenticated = authenticated;
        this.client.setAuthenticated(authenticated);
        if (!authenticated) {
            this.setSessionId(null);
            this.router.navigate(["/login"]);
        }
    }

    private updateSessionId(response: Response) {
        let sessionId = response.headers.get("x-evebox-session-id");
        if (sessionId && sessionId != this.sessionId) {
            console.log("Updating session ID from response header.");
            this.setSessionId(sessionId);
        }
    }

    private handle401(response: Response) {
        this.setAuthenticated(false);
    }

    /**
     * Low level options request, just fixup the URL.
     */
    _options(path: string) {
        return this.http.options(this.client.buildUrl(path));
    }

    request(method: string, path: string, options: RequestOptionsArgs = {}) {
        options.method = method;
        this.applySessionHeader(options);
        return this.http.request(this.client.buildUrl(path), options)
            .map((res: Response) => {
                this.updateSessionId(res);
                this.checkVersion(res);
                return res;
            })
            .catch((err: any) => {
                if (err.status === 401) {
                    this.handle401(err);
                }

                // Attempt to map the error to json.
                try {
                    return Observable.throw(err.json());
                }
                catch (e) {
                    return Observable.throw(err);
                }
            });
    }

    post(path: string, body: any, options: RequestOptionsArgs = {}) {
        options.body = JSON.stringify(body);
        return this.request("POST", path, options)
            .map((res: Response) => res.json())
            .toPromise();
    }

    get(path: string, options: RequestOptionsArgs = {}): Promise<any> {
        return this.request("GET", path, options)
            .map(res => res.json())
            .toPromise();
    }

    updateConfig() {
        return this.get("/api/1/config")
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

    logout() {
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

        let qsb: any = [];

        qsb.push(`agg=${agg}`);

        for (let option in options) {
            switch (option) {
                case "timeRange":
                    if (options[option] > 0) {
                        qsb.push(`timeRange=${options[option]}s`);
                    }
                    break;
                default:
                    qsb.push(`${option}=${options[option]}`);
                    break;
            }
        }

        return this.get(`api/1/report/agg?${qsb.join("&")}`);
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