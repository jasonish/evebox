// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { AfterViewChecked, Component, OnDestroy, OnInit } from "@angular/core";
import { ElasticSearchService } from "../elasticsearch.service";
import { ActivatedRoute, NavigationEnd, Router } from "@angular/router";
import { MousetrapService } from "../mousetrap.service";
import { TopNavService } from "../topnav.service";
import { AppEventCode, AppService, FEATURE_REPORTING } from "../app.service";
import { Subscription } from "rxjs";
import { ConfigService } from "../config.service";
import { ApiService } from "../api.service";
import { EVENT_TYPES } from "../shared/eventtypes";
import { ClientService } from "../client.service";
import $ from "jquery";

@Component({
    selector: "evebox-top-nav",
    templateUrl: "topnav.component.html",
})
export class TopNavComponent implements OnInit, OnDestroy, AfterViewChecked {
    isMenuCollapsed = true;

    routerSub: Subscription;

    features: any = {};

    EVENT_TYPES = EVENT_TYPES;

    reportsActive = false;

    reports = [
        { name: "Alerts", route: "/reports/alerts" },
        { name: "DNS", route: "/reports/dns" },
        { name: "SSH", route: "/reports/ssh" },
        { name: "Flow", route: "/reports/flow" },
        { name: "NetFlow", route: "/reports/netflow" },
        { name: "DHCP", route: "/reports/dhcp" },
    ];

    constructor(
        private router: Router,
        public elasticSearchService: ElasticSearchService,
        private mousetrap: MousetrapService,
        private topNavService: TopNavService,
        public appService: AppService,
        public client: ClientService,
        private api: ApiService,
        private route: ActivatedRoute,
        private configService: ConfigService
    ) {}

    ngOnInit() {
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
        this.mousetrap.bind(this, "\\", () => {
            let e = document.getElementById("timeRangeSelector");
            document.getElementById("timeRangeSelector").focus();
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
        $("#evebox-topnav a:not(.dropdown-toggle)").on("click", (e: any) => {
            this.isMenuCollapsed = true;
        });
    }

    gotoRoute(route: string) {
        this.router.navigate([route], { queryParams: {} });
    }

    timeRangeChanged($event: any) {
        (<HTMLElement>document.activeElement).blur();
        this.topNavService.setTimeRange($event.target.value);
        this.appService.dispatch({
            event: AppEventCode.TIME_RANGE_CHANGED,
            data: $event.target.value,
        });
    }

    showHelp() {
        this.appService.showHelp();
    }

    logout() {
        this.api.logout();
    }

    reload() {
        window.location.reload();
    }
}
