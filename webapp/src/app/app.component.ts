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

import {Component, OnInit} from "@angular/core";
import {AppService} from "./app.service";
import {AppEvent, AppEventService, AppEventType} from "./appevent.service";
import {ApiService} from "./api.service";

declare var document: any;
declare var window: any;

@Component({
    selector: "app-root",
    template: `
      <evebox-help *ngIf="isAuthenticated"></evebox-help>
      <evebox-top-nav *ngIf="isAuthenticated"></evebox-top-nav>
      <br/>
      <div class="container-fluid">
        <router-outlet></router-outlet>
      </div>
    `,
})
export class AppComponent implements OnInit {

    isAuthenticated: boolean = false;

    constructor(private appService: AppService,
                private api: ApiService,
                private appEventService: AppEventService) {
        this.isAuthenticated = this.api.isAuthenticated();
    }

    ngOnInit() {
        this.appEventService.subscribe((event: AppEvent) => {
            console.log("AppComponent: got event:");
            console.log(event);
            if (event.type == AppEventType.AUTHENTICATION_STATUS) {
                this.isAuthenticated = event.data.authenticated;
                console.log(`AppComponent.isAuthenticated: ${this.isAuthenticated}`);
            }
        });

        window.addEventListener("click", () => {
            this.appService.resetIdleTime();
        }, true);
    }

}
