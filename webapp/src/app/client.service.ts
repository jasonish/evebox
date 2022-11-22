// Copyright (C) 2018-2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE
// OR OTHER DEALINGS IN THE SOFTWARE.

import { Injectable } from "@angular/core";
import {
  HttpClient,
  HttpErrorResponse,
  HttpHeaders,
  HttpParams,
} from "@angular/common/http";
import { Observable, throwError } from "rxjs";
import { catchError, finalize, map } from "rxjs/operators";
import { BehaviorSubject } from "rxjs/internal/BehaviorSubject";
import { of } from "rxjs/internal/observable/of";
import { GITREV } from "../environments/gitrev";

const SESSION_HEADER = "x-evebox-session-id";

declare var localStorage: any;

export interface LoginResponse {
  session_id: string;
}

@Injectable()
export class ClientService {
  private versionWarned = false;

  private _baseUrl: string = window.location.pathname;

  private authenticated: boolean = false;

  public _sessionId: string = null;

  private _isAuthenticated$: BehaviorSubject<boolean> =
    new BehaviorSubject<boolean>(this.authenticated);

  public reloadRequired = false;

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
    console.log(`ClientService.setAuthenticated: ${authenticated}`);
    this.authenticated = authenticated;
    this.isAuthenticated$.next(authenticated);
  }

  setSessionId(sessionId: string | null) {
    console.log(`ClientService.setSessionId: sessionId = ${sessionId}`);
    this._sessionId = sessionId;
    localStorage._sessionId = this._sessionId;
  }

  get sessionId(): string | null {
    return this._sessionId;
  }

  get baseUrl(): string {
    return this._baseUrl;
  }

  get isAuthenticated$(): BehaviorSubject<boolean> {
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

    return this.http
      .get(this.buildUrl("api/1/config"), {
        observe: "response",
        headers: headers,
      })
      .pipe(
        map((response) => {
          let sessionId = response.headers.get("x-evebox-session-id");
          if (sessionId) {
            this._sessionId = sessionId;
          }
          this.setAuthenticated(true);
          return true;
        }),
        catchError((error) => {
          this.setAuthenticated(false);
          return of(false);
        })
      );
  }

  login(
    username: string = "",
    password: string = ""
  ): Observable<LoginResponse> {
    let params = new HttpParams()
      .append("username", username)
      .append("password", password);
    return this.http.post(this.buildUrl("api/1/login"), params).pipe(
      map((response: LoginResponse) => {
        console.log(`Got session ID: ${response.session_id}`);
        //                this.setAuthenticated(true);
        this.setSessionId(response.session_id);
        return response;
      }),
      catchError((error: any) => {
        this.setAuthenticated(false);
        this.setSessionId(null);
        return throwError(error);
      })
    );
  }

  logout(): Observable<boolean> {
    let headers = new HttpHeaders();
    if (this._sessionId) {
      headers = this.addSessionIdHeader(headers);
    }
    return this.http
      .get(this.buildUrl("api/1/logout"), { headers: headers })
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

  get(path: string, params?: HttpParams): Observable<any> {
    let headers = new HttpHeaders();
    if (this._sessionId) {
      headers = this.addSessionIdHeader(headers);
    }

    let options: any = {
      headers: headers,
      observe: "response",
    };

    if (params) {
      options.params = params;
    }

    return this.http.get(this.buildUrl(path), options).pipe(
      map((response: any) => {
        this.updateSessionId(response);
        this.checkVersion(response);
        return response.body;
      }),
      catchError((error: HttpErrorResponse) => {
        if (error.error instanceof ErrorEvent) {
          // Client side or network error.
        } else {
          if (error.status === 401) {
            this.handle401();
          }
        }
        return throwError(error);
      })
    );
  }

  public updateSessionId(response: any) {
    let sessionId = response.headers.get(SESSION_HEADER);
    if (sessionId && sessionId != this._sessionId) {
      console.log("Updating session ID from response header.");
      this.setSessionId(sessionId);
    }
  }

  private handle401() {
    this.setAuthenticated(false);
  }

  checkVersion(response: any): void {
    if (this.versionWarned) {
      return;
    }
    const webappRev: string = GITREV;
    const serverRev: string = response.headers.get("x-evebox-git-revision");
    if (webappRev !== serverRev) {
      console.log(
        `Client: server version: ${serverRev}; webapp version: ${webappRev}`
      );
      this.versionWarned = true;
      this.reloadRequired = true;
    }
  }
}
