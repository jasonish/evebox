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

import {CanActivate, Router, RouterModule, Routes} from '@angular/router';
import {EventsComponent} from './events.component';
import {EventComponent} from './event.component';
import {AlertsComponent} from './alerts.component';
import {AlertReportComponent} from './reports/alerts-report.component';
import {DNSReportComponent} from './reports/dns-report.component';
import {FlowReportComponent} from './reports/flow-report.component';
import {NetflowReportComponent} from './reports/netflow-report.component';
import {Injectable, ModuleWithProviders} from '@angular/core';
import {IpReportComponent} from './reports/ip-report/ip-report.component';
import {SshReportComponent} from './reports/ssh-report.component';
import {LoginComponent} from './login/login.component';
import {AppService} from './app.service';
import {ConfigService} from './config.service';
import {AdminComponent} from './admin/admin.component';
import {UsersComponent} from './admin/users/users.component';
import {ApiService} from "app/api.service";

declare var window: any;

@Injectable()
export class AuthGuard implements CanActivate {

    constructor(private api: ApiService) {
    }

    canActivate() {
        if (this.api.isAuthenticated()) {
            return Promise.resolve(true);
        }
        return this.api.login()
            .then(() => true)
            .catch(() => false);
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

        return this.api.get("/api/1/config")
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

const adminRoutes: Routes = [
    {
        path: "",
        pathMatch: "prefix",
        component: AdminComponent,
    },
    {
        path: "users",
        pathMatch: "prefix",
        component: UsersComponent,
    }
];

const routes: Routes = [

    {
        path: 'login',
        pathMatch: 'prefix',
        component: LoginComponent,
    },
    {
        path: '',
        pathMatch: 'prefix',
        canActivate: [AuthGuard],
        children: [
            {
                path: "admin",
                pathMatch: "prefix",
                children: adminRoutes,
            },
            {
                path: '',
                redirectTo: 'inbox',
                pathMatch: 'prefix',
            },
            {
                path: 'inbox', component: AlertsComponent, pathMatch: 'prefix',
            },
            {
                path: 'inbox', component: AlertsComponent, pathMatch: 'prefix',
            }
            ,
            {
                path: 'escalated',
                component: AlertsComponent,
                pathMatch: 'prefix',
            }
            ,
            {
                path: 'alerts', component: AlertsComponent, pathMatch: 'prefix',
            }
            ,
            {
                path: 'event/:id',
                component: EventComponent,
                pathMatch: 'prefix',
            }
            ,
            {
                path: 'events', component: EventsComponent, pathMatch: 'prefix',
            }
            ,
            {path: 'reports/alerts', component: AlertReportComponent},
            {path: 'reports/dns', component: DNSReportComponent},
            {path: 'reports/flow', component: FlowReportComponent},
            {path: 'reports/netflow', component: NetflowReportComponent},
            {path: 'reports/ssh', component: SshReportComponent},
            {
                path: 'reports/ip',
                component: IpReportComponent,
                pathMatch: 'prefix',
            },
        ]
    },
];

export const routing: ModuleWithProviders = RouterModule.forRoot(routes, {useHash: true});
