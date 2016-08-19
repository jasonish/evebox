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

import {RouterConfig, provideRouter} from "@angular/router";
import {EventsComponent} from "./events.component";
import {EventComponent} from "./event.component";
import {AlertsComponent} from "./alerts.component";
import {AlertReportComponent} from "./reports/alerts-report.component";
import {DNSReportComponent} from "./reports/dns-report.component";
import {FlowReportComponent} from "./reports/flow-report.component";
import {NetflowReportComponent} from "./reports/netflow-report.component";

export const routes:RouterConfig = [
    {
        path: "inbox", component: AlertsComponent, pathMatch: "prefix",
    }
    ,
    {
        path: "escalated", component: AlertsComponent, pathMatch: "prefix",
    }
    ,
    {
        path: "alerts", component: AlertsComponent, pathMatch: "prefix",
    }
    ,
    {
        path: "event/:id", component: EventComponent, pathMatch: "prefix",
    }
    ,
    {
        path: "events", component: EventsComponent, pathMatch: "prefix",
    }
    ,
    {path: "reports/alerts", component: AlertReportComponent},
    {path: "reports/dns", component: DNSReportComponent},
    {path: "reports/flow", component: FlowReportComponent},
    {path: "reports/netflow", component: NetflowReportComponent},
    // Let the inbox by the default route.
    {
        path: "", redirectTo: "inbox", pathMatch: "prefix"
    }
];

export const APP_ROUTER_PROVIDERS = [
    provideRouter(routes)
];
