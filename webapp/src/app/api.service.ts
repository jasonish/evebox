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

import {Injectable} from '@angular/core';
import {Headers, Http, RequestOptionsArgs, Response} from '@angular/http';
import {ToastrService} from './toastr.service';
import {GITREV} from '../environments/gitrev';
import {Router} from '@angular/router';

import {Observable} from 'rxjs/Rx';
import 'rxjs/add/operator/catch';
import 'rxjs/add/operator/map';

export class QueryStringBuilder {

    keys: any = {};

    set(key: string, value: any) {
        this.keys[key] = value;
    }

    build() {
        let parts: any = [];

        for (let key in this.keys) {
            parts.push(`${key}=${this.keys[key]}`);
        }

        return parts.join('&');
    }
}

@Injectable()
export class ApiService {

    private baseUrl: string = window.location.pathname;

    private versionWarned = false;

    private sessionId: string;

    constructor(private http: Http, private toastr: ToastrService, private router: Router) {
    }

    setSessionId(sessionId: string) {
        this.sessionId = sessionId;
    }

    hasSessionId(): boolean {
        return this.sessionId !== undefined &&
            this.sessionId !== "";
    }

    getSessionId(): string {
        return this.sessionId;
    }

    checkVersion(response: Response) {
        if (this.versionWarned) {
            return;
        }
        let webappRev: string = GITREV;
        let serverRev: string = response.headers.get('x-evebox-git-revision');
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
            headers.append('x-evebox-session-id', this.sessionId);
            options.headers = headers;
        }
    }

    request(method: string, path: string, options: RequestOptionsArgs = {}) {
        let url = `${this.baseUrl}${path.replace(/^\//, '')}`;
        options.method = method;
        this.applySessionHeader(options);
        return this.http.request(url, options)
            .map((res: Response) => {
                let sessionId = res.headers.get("x-evebox-session-id");
                if (sessionId && sessionId != this.sessionId) {
                    console.log("Updating session ID from response header.");
                    this.setSessionId(sessionId);
                }
                return res;
            });
    }

    post(path: string, body: any, options: RequestOptionsArgs = {}) {
        options.body = JSON.stringify(body);
        return this.request("POST", path, options)
            .map((res: Response) => res.json())
            .toPromise();
    }

    postForm(path: string, form: URLSearchParams, options: RequestOptionsArgs = {}) {
        options.body = form.toString();
        this.applySessionHeader(options);
        let headers = options.headers || new Headers();
        headers.append('Content-Type',
            'application/x-www-form-urlencoded');
        options.headers = headers;
        return this.request("POST", path, options)
            .map((res: Response) => res.json())
            .toPromise();
    }

    get(path: string, options: RequestOptionsArgs = {}): Promise<any> {
        return this.request("GET", path, options)
            .catch((err: any) => {
                if (err.status == 401) {
                    console.log("got 401, redirecting to login page");
                    this.router.navigate(['/login']);
                }
                return Observable.throw(err);
            })
            .toPromise()
            .then((res: Response) => {
                this.checkVersion(res);
                return res.json();
            })
            .then((response: any) => {
                return response;
            }, (response: any) => {
                let error: any;
                try {
                    error = JSON.parse(response._body);
                    console.log("parsed error ersponse");
                } catch (err) {
                    console.log('Failed to parse response body.');
                    console.log(err);
                    error = response;
                }
                throw error;
            });
    }

    login(username: string, password?: string) {
        let params = new URLSearchParams();
        params.set("username", username);
        return this.postForm("/api/1/login", params)
            .then(response => {
                this.setSessionId(response.session_id);
                return true;
            });
    }

    getWithParams(path: string, params = {}): Promise<any> {

        let qsb: any = [];

        for (let param in params) {
            qsb.push(`${param}=${params[param]}`);
        }

        return this.get(`${path}?${qsb.join('&')}`);
    }

    getVersion() {
        return this.get('api/1/version');
    }

    eventToPcap(what: any, event: any) {

        let form = document.createElement('form');
        form.setAttribute('method', 'post');
        form.setAttribute('action', 'api/1/eve2pcap');

        let whatField = document.createElement('input');
        whatField.setAttribute('type', 'hidden');
        whatField.setAttribute('name', 'what');
        whatField.setAttribute('value', what);
        form.appendChild(whatField);

        let eventField = document.createElement('input');
        eventField.setAttribute('type', 'hidden');
        eventField.setAttribute('name', 'event');
        eventField.setAttribute('value', JSON.stringify(event));
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

        return this.get(`api/1/report/histogram?${query.join('&')}`);
    }

    reportAgg(agg: string, options: ReportAggOptions = {}) {

        let qsb: any = [];

        qsb.push(`agg=${agg}`);

        for (let option in options) {
            switch (option) {
                case 'timeRange':
                    if (options[option] > 0) {
                        qsb.push(`timeRange=${options[option]}s`);
                    }
                    break;
                default:
                    qsb.push(`${option}=${options[option]}`);
                    break;
            }
        }

        return this.get(`api/1/report/agg?${qsb.join('&')}`);
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