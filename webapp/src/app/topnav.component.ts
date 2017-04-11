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

import {
    Component, OnInit, OnDestroy, OnChanges,
    AfterViewChecked
} from '@angular/core';
import {ElasticSearchService} from './elasticsearch.service';
import {Router, ActivatedRoute} from '@angular/router';
import {MousetrapService} from './mousetrap.service';
import {TopNavService} from './topnav.service';
import {AppService, AppEventCode, FEATURE_REPORTING} from './app.service';
import {Subscription} from 'rxjs/Rx';
import {ConfigService} from './config.service';

declare var $: any;

@Component({
    selector: 'evebox-top-nav',
    template: `<nav class="navbar navbar-default navbar-fixed-top">
  <div class="container-fluid">
    <div class="navbar-header">
      <button type="button" class="navbar-toggle collapsed"
              data-toggle="collapse" data-target="#bs-example-navbar-collapse-1"
              aria-expanded="false">
        <span class="sr-only">Toggle navigation</span>
        <span class="icon-bar"></span>
        <span class="icon-bar"></span>
        <span class="icon-bar"></span>
        <span class="icon-bar"></span>
      </button>
      <a class="navbar-brand" href="#/">EveBox</a>
    </div>

    <div class="collapse navbar-collapse">
      <ul class="nav navbar-nav">
        <li [ngClass]="{active: isActive('/inbox')}"><a
            href="#/inbox">Inbox</a></li>
        <li [ngClass]="{active: isActive('/escalated')}"><a
            href="#/escalated">Escalated</a></li>
        <li [ngClass]="{active: isActive('/alerts')}"><a
            href="#/alerts">Alerts</a></li>
        <li [ngClass]="{active: isActive('/events')}"><a
            href="#/events">Events</a></li>

        <li *ngIf="features['reporting']" [ngClass]="{active: isActive('/reports')}" class="dropdown">
          <a href="#" class="dropdown-toggle" data-toggle="dropdown"
             role="button" aria-haspopup="true" aria-expanded="false">Reports
            <span class="caret"></span></a>
          <ul class="dropdown-menu">
            <li><a href="#/reports/alerts">Alerts</a></li>
            <li><a href="#/reports/dns">DNS</a></li>
            <li><a href="#/reports/netflow">Netflow</a></li>
            <li><a href="#/reports/flow">Flow</a></li>
            <li><a href="#/reports/ssh">SSH</a></li>
          </ul>
        </li>

      </ul>

      <ul class="nav navbar-nav navbar-right">
        <li><a href="javascript:void(0);" (click)="showHelp()">Help</a></li>

        <li>
          <a href="#" class="dropdown-toggle" data-toggle="dropdown"><span
              class="glyphicon glyphicon-cog"></span></a>
          <ul class="dropdown-menu">
            <li><a href="javascript:void(0)" (click)="setTheme('default')">Light (Default)</a></li>
            <li><a href="javascript:void(0)" (click)="setTheme('slate')">Slate</a></li>
            <li role="separator" class="divider"></li>
            <li><a href="#/admin">Admin</a></li>
          </ul>
        </li>

        <li>
          <a><span class="badge">{{elasticSearchService.jobSize()}}</span></a>
        </li>
      </ul>

      <form name="dateSelectorForm" class="navbar-form navbar-right">
        <select *ngIf="!appService.isTimeRangeDisabled()" class="form-control"
                [ngModel]="topNavService.timeRange" name="timeRange"
                (change)="timeRangeChanged($event)">
          <option value="1m">Last minute</option>
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
})
export class TopNavComponent implements OnInit, OnDestroy, AfterViewChecked {
    appService: AppService;

    routerSub: Subscription;

    features: any = {};

    elasticSearchService: ElasticSearchService;

    constructor(private router: Router,
                elasticSearchService: ElasticSearchService,
                private mousetrap: MousetrapService,
                private topNavService: TopNavService,
                appService: AppService,
                private configService: ConfigService) {
        this.elasticSearchService = elasticSearchService;
        this.appService = appService;
    }

    ngOnInit() {

        console.log("TopNavService.ngOnInit");

        if (this.configService.hasFeature(FEATURE_REPORTING)) {
            this.features['reporting'] = true;
        }

        this.mousetrap.bind(this, 'g i', () => {
            this.gotoRoute('/inbox');
        });
        this.mousetrap.bind(this, 'g x', () => {
            this.gotoRoute('/escalated');
        });
        this.mousetrap.bind(this, 'g a', () => {
            this.gotoRoute('/alerts');
        });
        this.mousetrap.bind(this, 'g e', () => {
            this.gotoRoute('/events');
        });
        this.mousetrap.bind(this, '?', () => {
            this.showHelp();
        });

        // Re-enable the time picker after each route change.
        this.routerSub = this.router.events.subscribe((event) => {

            switch (this.appService.getRoute()) {
                case '/escalated':
                case '/event':
                    this.appService.disableTimeRange();
                    break;
                default:
                    this.appService.enableTimeRange();
                    break;
            }

        });
    }

    ngOnDestroy(): any {
        this.mousetrap.unbind(this);
        this.routerSub.unsubscribe();
    }

    ngAfterViewChecked() {
        $('.dropdown-toggle').dropdown();
    }

    gotoRoute(route: string) {
        this.router.navigate([route], {queryParams: {}});
    }

    timeRangeChanged($event: any) {
        this.topNavService.setTimeRange($event.target.value);
        this.appService.dispatch({
            event: AppEventCode.TIME_RANGE_CHANGED,
            data: $event.target.value
        });
    }

    isActive(route: any) {
        return route == this.appService.getRoute();
    }

    showHelp() {
        this.appService.dispatch({
            event: AppEventCode.SHOW_HELP
        });
    }

    setTheme(name: string) {
        // Pass off to appService.
        this.appService.setTheme(name);
    }
}