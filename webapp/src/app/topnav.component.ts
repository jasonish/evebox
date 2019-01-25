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

import {AfterViewChecked, Component, OnDestroy, OnInit} from "@angular/core";
import {ElasticSearchService} from "./elasticsearch.service";
import {ActivatedRoute, NavigationEnd, Router} from "@angular/router";
import {MousetrapService} from "./mousetrap.service";
import {TopNavService} from "./topnav.service";
import {AppEventCode, AppService, FEATURE_REPORTING} from "./app.service";
import {Subscription} from "rxjs";
import {ConfigService} from "./config.service";
import {ApiService} from "./api.service";
import {EVENT_TYPES} from './shared/eventtypes';

declare var $: any;

@Component({
    selector: "evebox-top-nav",
    templateUrl: "topnav.component.html",
})
export class TopNavComponent implements OnInit, OnDestroy, AfterViewChecked {

    routerSub: Subscription;

    features: any = {};

    EVENT_TYPES = EVENT_TYPES;

    reportsActive = false;

    constructor(private router: Router,
                public elasticSearchService: ElasticSearchService,
                private mousetrap: MousetrapService,
                private topNavService: TopNavService,
                public appService: AppService,
                private api: ApiService,
                private route: ActivatedRoute,
                private configService: ConfigService) {
    }

    ngOnInit() {

        console.log("TopNavService.ngOnInit");

        if (this.configService.hasFeature(FEATURE_REPORTING)) {
            this.features["reporting"] = true;
        }

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
        this.routerSub = this.router.events.subscribe((event) => {
            if (event instanceof NavigationEnd) {
                this.reportsActive = event.url.startsWith("/reports/");
                this.toggleTimeRange();
            }
        });
        this.toggleTimeRange();

        this.reportsActive = this.router.url.startsWith("/reports/");
    }

    private toggleTimeRange() {
        switch (this.appService.getRoute()) {
            case "/escalated":
            case "/event":
                this.appService.disableTimeRange();
                break;
            default:
                this.appService.enableTimeRange();
                break;
        }
    }

    ngOnDestroy(): any {
        this.mousetrap.unbind(this);
        this.routerSub.unsubscribe();
    }

    ngAfterViewChecked() {
        // This makes the navbar collapse when a link is clicked. Only applies
        // when the viewport is narrow enough to make it collapse.
        $("#evebox-topnav-collapse-1 a:not(.no-collapse)").on("click", (e: any) => {
            var clickover = $(e.target);
            var $navbar = $(".navbar-collapse");
            var _opened = $navbar.hasClass("in");
            if (_opened === true && !clickover.hasClass("navbar-toggle")) {
                $navbar.collapse("hide");
            }
        });
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

    showHelp() {
        this.appService.dispatch({
            event: AppEventCode.SHOW_HELP
        });
    }

    logout() {
        this.api.logout();
    }
}
