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

import {Component, OnInit, OnDestroy, OnChanges} from "@angular/core";
import {ElasticSearchService} from "./elasticsearch.service";
import {ROUTER_DIRECTIVES, Router, ActivatedRoute} from "@angular/router";
import {MousetrapService} from "./mousetrap.service";
import {TopNavService} from "./topnav.service";
import {AppService, AppEventCode} from "./app.service";

@Component({
    selector: "evebox-top-nav",
    template: `<nav class="navbar navbar-default">
  <div class="container-fluid">
    <div class="navbar-header">
      <button type="button" class="navbar-toggle collapsed"
              data-toggle="collapse" data-target="#bs-example-navbar-collapse-1"
              aria-expanded="false">
        <span class="sr-only">Toggle navigation</span>
        <span class="icon-bar"></span>
        <span class="icon-bar"></span>
        <span class="icon-bar"></span>
      </button>
      <a class="navbar-brand" href="#/">EveBox</a>
    </div>

    <div class="collapse navbar-collapse">
      <ul class="nav navbar-nav">
        <li [ngClass]="{active: isActive('/inbox')}"><a
            [routerLink]="['/inbox']">Inbox</a></li>
        <li [ngClass]="{active: isActive('/escalated')}"><a
            [routerLink]="['/escalated']">Escalated</a></li>
        <li [ngClass]="{active: isActive('/alerts')}"><a
            [routerLink]="['/alerts']">Alerts</a></li>
        <li [ngClass]="{active: isActive('/events')}"><a
            [routerLink]="['/events']">Events</a></li>
      </ul>


      <ul class="nav navbar-nav navbar-right">
        <li><a href="javascript:void(0);" (click)="showHelp()">Help</a></li>
        <li>
          <a><span class="badge">{{elasticSearchService.jobSize()}}</span></a>
        </li>
      </ul>

      <form class="navbar-form navbar-right">
        <select *ngIf="!appService.isTimeRangeDisabled()" class="form-control" [ngModel]="topNavService.timeRange" (change)="timeRangeChanged($event)">
          <option value="1h">Last hour</option>
          <option value="3h">Last 3 hours</option>
          <option value="6h">Last 6 hours</option>
          <option value="12h">Last 12 hours</option>
          <option value="24h">Last 24 hours</option>
          <option value="3d">Last 3 days</option>
          <option value="7d">Last week</option>
          <option value="">All</option>
        </select>
      </form>

    </div>

  </div>
</nav>`,
    directives: [ROUTER_DIRECTIVES]
})
export class TopNavComponent implements OnInit, OnDestroy {

    constructor(private router:Router,
                private elasticSearchService:ElasticSearchService,
                private mousetrap:MousetrapService,
                private topNavService:TopNavService,
                private appService:AppService) {
    }

    ngOnInit() {

        this.mousetrap.bind(this, "g i", () => {
            this.gotoRoute("/inbox");
        });
        this.mousetrap.bind(this, "g x", () => {
            this.gotoRoute("/escalated");
        });
        this.mousetrap.bind(this, "g a", () => {
            this.gotoRoute("/alerts");
        });
        this.mousetrap.bind(this, "g e", () => {
            this.gotoRoute("/events");
        });
        this.mousetrap.bind(this, "?", () => {
            this.showHelp();
        });

        // Re-enable the time picker after each route change.
        this.router.events.subscribe((event) => {
            this.appService.enableTimeRange();
        });

    }

    ngOnDestroy():any {
        this.mousetrap.unbind(this);
    }

    gotoRoute(route:string) {
        this.router.navigate([route], {queryParams: {}});
    }

    timeRangeChanged($event:any) {
        this.topNavService.timeRange = $event.target.value;
        this.appService.dispatch({
            event: AppEventCode.TIME_RANGE_CHANGED,
            data: $event.target.value
        });
    }

    isActive(route:any) {
        return route == this.appService.getRoute();
    }

    showHelp() {
        this.appService.dispatch({
            event: AppEventCode.SHOW_HELP
        });
    }
}