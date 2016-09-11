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

import "core-js/es6";
import "reflect-metadata";
import "@angular/platform-browser";
import "@angular/platform-browser-dynamic";
import "@angular/core";
import "@angular/common";
import "@angular/http";
import "@angular/router";
import "rxjs";
import "bootstrap/dist/css/bootstrap.css";
import "./evebox.scss";
import {platformBrowserDynamic} from "@angular/platform-browser-dynamic";
import {HttpModule} from "@angular/http";
import {RouterModule} from "@angular/router";
import {enableProdMode, NgModule} from "@angular/core";
import {BrowserModule} from "@angular/platform-browser";
import {FormsModule} from "@angular/forms";
import {ConfigService} from "./config.service";
import {ElasticSearchService} from "./elasticsearch.service";
import {MousetrapService} from "./mousetrap.service";
import {TopNavService} from "./topnav.service";
import {AlertService} from "./alert.service";
import {AppService} from "./app.service";
import {ApiService} from "./api.service";
import {EventServices} from "./eventservices.service";
import {EventService} from "./event.service";
import {ToastrService} from "./toastr.service";
import {ReportsService} from "./reports/reports.service";
import {EveboxSubscriptionService} from "./subscription.service";
import {EveboxHumanizeService} from "./humanize.service";
import {EveboxFormatIpAddressPipe} from "./pipes/format-ipaddress.pipe";
import {AppComponent} from "./app.component";
import {AlertsComponent} from "./alerts.component";
import {EventsComponent} from "./events.component";
import {EventComponent} from "./event.component";
import {
    DNSReportComponent,
    EveboxReportDataTable
} from "./reports/dns-report.component";
import {AlertReportComponent} from "./reports/alerts-report.component";
import {NetflowReportComponent} from "./reports/netflow-report.component";
import {FlowReportComponent} from "./reports/flow-report.component";
import {routing} from "./app.routes";
import {EveboxHumanizePipe} from "./pipes/humanize.pipe";
import {EveboxEventTable2Component} from "./eventtable2.component";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";
import {EveboxMetricsGraphicComponent} from "./metricgraphics.component";
import {EveboxHelpComponent} from "./help.component";
import {TopNavComponent} from "./topnav.component";
import {EveboxHexPipe} from "./pipes/hex.pipe";
import {EveBoxEventDescriptionPrinterPipe} from "./pipes/eventdescription.pipe";
import {EventSeverityToBootstrapClass} from "./pipes/event-severity-to-bootstrap-class.pipe";
import {EveBoxGenericPrettyPrinter} from "./pipes/generic-pretty-printer.pipe";
import {JsonPipe} from "./pipes/json.pipe";
import {EveboxMapToItemsPipe} from "./pipes/maptoitems.pipe";
import {EveboxBase64DecodePipe} from "./pipes/base64decode.pipe";
import {EveboxFormatTimestampPipe} from "./pipes/format-timestamp.pipe";
import {AceEditor} from "./ace-editor.component";
import {EveboxSearchLinkComponent} from "./search-link.component";
import {KeyTableDirective} from "./keytable.directive";
import {EveboxDurationComponent} from "./duration.component";
import {AlertTableComponent} from "./alert-table.component";
import {EveboxEventTableComponent} from "./event-table.component";
require("zone.js/dist/zone");

// Raw imports.
require("!!script!jquery/dist/jquery.min.js");
require("!!script!bootstrap/dist/js/bootstrap.min.js");

if (process.env.ENV == "production") {
    console.log("Enabling production mode.");
    enableProdMode();
}

@NgModule({
    imports: [
        BrowserModule,
        FormsModule,
        RouterModule,
        HttpModule,
        routing,
    ],
    declarations: [
        AppComponent,
        AlertsComponent,
        EventsComponent,
        EventComponent,
        DNSReportComponent,
        AlertReportComponent,
        NetflowReportComponent,
        FlowReportComponent,
        EveboxMetricsGraphicComponent,
        EveboxReportDataTable,
        EveboxLoadingSpinnerComponent,
        EveboxEventTable2Component,
        TopNavComponent,
        EveboxHelpComponent,
        AceEditor,
        EveboxSearchLinkComponent,
        EveboxDurationComponent,
        KeyTableDirective,
        AlertTableComponent,
        EveboxLoadingSpinnerComponent,
        KeyTableDirective,
        EveboxDurationComponent,
        EveboxEventTableComponent,
        EveboxLoadingSpinnerComponent,
        KeyTableDirective,
        EveboxDurationComponent,
        EveboxLoadingSpinnerComponent,
        EveboxSearchLinkComponent,
        EveboxReportDataTable,
        EveboxMetricsGraphicComponent,
        EveboxLoadingSpinnerComponent,
        EveboxSearchLinkComponent,
        EveboxMetricsGraphicComponent,
        EveboxLoadingSpinnerComponent,
        EveboxReportDataTable,
        EveboxEventTable2Component,
        EveboxMetricsGraphicComponent,
        EveboxReportDataTable,
        EveboxLoadingSpinnerComponent,
        EveboxFormatIpAddressPipe,
        EveboxMapToItemsPipe,
        EveboxFormatIpAddressPipe,
        EveboxHumanizePipe,
        EventSeverityToBootstrapClass,
        JsonPipe,
        EveboxMapToItemsPipe,
        EveBoxGenericPrettyPrinter,
        EveBoxEventDescriptionPrinterPipe,
        EveboxBase64DecodePipe,
        EveboxHexPipe,
        EveboxFormatIpAddressPipe,
        EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EventSeverityToBootstrapClass,
        EveBoxEventDescriptionPrinterPipe,
        EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EveBoxEventDescriptionPrinterPipe,
        EventSeverityToBootstrapClass,
        EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EveBoxEventDescriptionPrinterPipe,
        EventSeverityToBootstrapClass,

    ],
    providers: [
        ConfigService,
        ElasticSearchService,
        MousetrapService,
        TopNavService,
        AlertService,
        AppService,
        ApiService,
        EventServices,
        EventService,
        ToastrService,
        ReportsService,
        EveboxSubscriptionService,
        EveboxHumanizeService,
        EveboxFormatIpAddressPipe,
    ],
    bootstrap: [
        AppComponent,
    ]
})
export class AppModule {
}

platformBrowserDynamic().bootstrapModule(AppModule);
