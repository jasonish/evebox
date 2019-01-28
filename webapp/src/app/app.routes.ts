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

import {CanActivate, RouterModule, Routes} from "@angular/router";
import {EventsComponent} from "./events/events.component";
import {EventComponent} from "./event/event.component";
import {AlertsComponent} from "./alerts.component";
import {AlertReportComponent} from "./reports/alerts-report.component";
import {DNSReportComponent} from "./reports/dns-report/dns-report.component";
import {FlowReportComponent} from "./reports/flow-report.component";
import {NetflowReportComponent} from "./reports/netflow-report.component";
import {Injectable, ModuleWithProviders} from "@angular/core";
import {IpReportComponent} from "./reports/ip-report/ip-report.component";
import {SshReportComponent} from "./reports/ssh-report.component";
import {LoginComponent} from "./login/login.component";
import {ConfigService} from "./config.service";
import {ApiService} from "app/api.service";
import {SettingsComponent} from "./settings/settings.component";
import {DebugComponent} from "./debug/debug.component";
import {Alert} from "selenium-webdriver";

declare var window: any;

@Injectable()
export class AuthGuard implements CanActivate {

    constructor(private api: ApiService) {
    }

    canActivate() {
        if (this.api.isAuthenticated()) {
            return Promise.resolve(true);
        }
        return this.api.checkAuth();
    }
}

@Injectable()
export class NeverActivate implements CanActivate {
    canActivate() {
        return false;
    }
}

@Injectable()
export class ConfigResolver implements CanActivate {

    constructor(private api: ApiService,
                private configService: ConfigService) {
    }

    canActivate(): Promise<boolean> {
        if (this.configService.hasConfig()) {
            return Promise.resolve(true);
        }

        return this.api.get("api/1/config")
                .then((config) => {
                    console.log(config);
                    this.configService.setConfig(config);
                    return true;
                })
                .catch(() => {
                    return false;
                });
    }
}

const routes: Routes = [

    {
        path: "login",
        pathMatch: "prefix",
        component: LoginComponent,
    },
    {
        path: "debug",
        pathMatch: "prefix",
        component: DebugComponent,
    },
    {
        path: "",
        pathMatch: "prefix",
        canActivate: [AuthGuard],
        children: [
            {
                path: "",
                redirectTo: "inbox",
                pathMatch: "prefix",
            },
            {
                path: "inbox", component: AlertsComponent, pathMatch: "prefix",
            },
            {
                path: "inbox", component: AlertsComponent, pathMatch: "prefix",
            }
            ,
            {
                path: "escalated",
                component: AlertsComponent,
                pathMatch: "prefix",
            }
            ,
            {
                path: "alerts", component: AlertsComponent, pathMatch: "prefix",
            }
            ,
            {
                path: "event/:id",
                component: EventComponent,
                pathMatch: "prefix",
            }
            ,
            {
                path: "events", component: EventsComponent, pathMatch: "prefix",
            }
            ,
            {
                path: "reports",
                children: [
                    // The "reports/" route. Never allow to activate as there
                    // is nothing here.
                    {
                        path: "",
                        canActivate: [NeverActivate],
                        component: AlertReportComponent,
                    },
                    {
                        path: "alerts",
                        component: AlertReportComponent,
                    },
                    {
                        path: "dns",
                        component: DNSReportComponent
                    },
                    {
                        path: "flow",
                        component: FlowReportComponent
                    },
                    {
                        path: "netflow",
                        component: NetflowReportComponent
                    },
                    {
                        path: "ssh",
                        component: SshReportComponent
                    },
                ]
            },
            {
                path: "reports/ip",
                component: IpReportComponent,
                pathMatch: "prefix",
            },

            {
                path: "settings", component: SettingsComponent,
            }
        ]
    },
];

export const routing: ModuleWithProviders = RouterModule.forRoot(routes, {useHash: true});
