/* Copyright (c) 2018 Jason Ish
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
import {HttpClient, HttpHeaders, HttpParams} from "@angular/common/http";
import {Observable} from "rxjs/Observable";
import {of} from "rxjs/observable/of";
import {catchError, finalize, map} from "rxjs/operators";
import {_throw} from "rxjs/observable/throw";
import {BehaviorSubject} from "rxjs/BehaviorSubject";

declare var localStorage: any;

export interface LoginResponse {
    session_id: string;
}

@Injectable()
export class ClientService {

    private _baseUrl: string = window.location.pathname;

    private authenticated: boolean = false;

    private _sessionId: string = null;

    private _isAuthenticated$: BehaviorSubject<boolean> =
        new BehaviorSubject<boolean>(this.authenticated);

    constructor(private http: HttpClient) {
        if (localStorage._sessionId) {
            console.log("Restoring session-id from local storage.");
            this._sessionId = localStorage._sessionId;
        }
    }

    buildUrl(path: string): string {
        let url = `${this._baseUrl}${path.replace(/^\//, "")}`;
        return url;
    }

    setAuthenticated(authenticated: boolean) {
        this.authenticated = authenticated;
        this.isAuthenticated$.next(authenticated);
    }

    setSessionId(sessionId: string | null) {
        this._sessionId = sessionId;
        localStorage._sessionId = this._sessionId;
    }

    get sessionId(): string | null {
        return this.sessionId;
    }

    get baseUrl(): string {
        return this._baseUrl;
    }

    get isAuthenticated$():BehaviorSubject<boolean> {
        return this._isAuthenticated$;
    }

    addSessionIdHeader(headers: HttpHeaders | null): HttpHeaders {
        if (headers === null) {
            headers = new HttpHeaders();
        }
        if (this._sessionId) {
            headers = headers.append("x-evebox-session-id", this._sessionId);
        }
        return headers;
    }

    /**
     * Check if the client is already authenticated.
     *
     * @returns {Observable<boolean>}
     */
    checkAuthentication(): Observable<boolean> {
        let headers = new HttpHeaders();
        if (this._sessionId) {
            headers = this.addSessionIdHeader(headers);
        }

        return this.http.get(this.buildUrl("api/1/config"), {
            observe: "response",
            headers: headers
        }).map((response) => {
            let sessionId = response.headers.get("x-evebox-session-id");
            if (sessionId) {
                this._sessionId = sessionId;
            }
            this.setAuthenticated(true);
            return true;
        }).catch((error) => {
            this.setAuthenticated(false);
            return of(false);
        });
    }

    login(username: string = "", password: string = ""): Observable<LoginResponse> {
        let params = new HttpParams()
            .append("username", username)
            .append("password", password);
        return this.http.post(this.buildUrl("api/1/login"), params)
            .pipe(
                map((response: LoginResponse) => {
                    console.log(`Got session ID: ${response.session_id}`);
                    this.setAuthenticated(true);
                    this.setSessionId(response.session_id);
                    return response;
                }),
                catchError((error: any) => {
                    this.setAuthenticated(false);
                    this.setSessionId(null);
                    return _throw(error);
                })
            );
    }

    logout(): Observable<boolean> {
        let headers = new HttpHeaders();
        if (this._sessionId) {
            headers = this.addSessionIdHeader(headers);
        }
        return this.http.get(this.buildUrl("api/1/logout"), {headers: headers})
            .pipe(
                map(() => {
                    return true;
                }),
                catchError((error: any) => {
                    console.log("logout error:");
                    console.log(error);
                    return of(true);
                }),
                finalize(() => {
                    console.log("Clearing session ID.");
                    this.setAuthenticated(false);
                    this.setSessionId(null);
                })
            );
    }
}
