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

import {BrowserModule} from '@angular/platform-browser';
import {NgModule} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {HttpModule} from '@angular/http';

import {AppComponent} from './app.component';
import {RouterModule} from "@angular/router";
import {routing} from "./app.routes";
import {AlertsComponent} from "./alerts.component";
import {EventComponent} from "./event.component";
import {EventsComponent} from "./events.component";
import {DNSReportComponent} from "./reports/dns-report.component";
import {AlertReportComponent} from "./reports/alerts-report.component";
import {NetflowReportComponent} from "./reports/netflow-report.component";
import {FlowReportComponent} from "./reports/flow-report.component";
import {EveboxMetricsGraphicComponent} from "./metricgraphics.component";
import {EveboxReportDataTable} from "./reports/report-data-table";
import {IpReportComponent} from "./reports/ip-report/ip-report.component";
import {EveboxFilterInputComponent} from "./shared/filter-input.component";
import {TopNavComponent} from "./topnav.component";
import {EveboxHelpComponent} from "./help.component";
import {AceEditor} from "./ace-editor.component";
import {AlertTableComponent} from "./alert-table.component";
import {EveboxEventTableComponent} from "./event-table.component";
import {KeyTableDirective} from "./keytable.directive";
import {EveboxDurationComponent} from "./duration.component";
import {EveboxSearchLinkComponent} from "./search-link.component";
import {EveboxEventTable2Component} from "./eventtable2.component";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";
import {EveboxFormatIpAddressPipe} from "./pipes/format-ipaddress.pipe";
import {EveboxMapToItemsPipe} from "./pipes/maptoitems.pipe";
import {EveBoxGenericPrettyPrinter} from "./pipes/generic-pretty-printer.pipe";
import {EveBoxEventDescriptionPrinterPipe} from "./pipes/eventdescription.pipe";
import {EveboxJsonPrettyPipe} from "./pipes/json.pipe";
import {EveboxHumanizePipe} from "./pipes/humanize.pipe";
import {EveboxBase64DecodePipe} from "./pipes/base64decode.pipe";
import {EveboxHexPipe} from "./pipes/hex.pipe";
import {EventSeverityToBootstrapClass} from "./pipes/event-severity-to-bootstrap-class.pipe";
import {EveboxFormatTimestampPipe} from "./pipes/format-timestamp.pipe";
import {ConfigService} from "./config.service";
import {ElasticSearchService} from "./elasticsearch.service";
import {MousetrapService} from "./mousetrap.service";
import {TopNavService} from "./topnav.service";
import {AppService} from "./app.service";
import {AlertService} from "./alert.service";
import {EventService} from "./event.service";
import {EventServices} from "./eventservices.service";
import {ToastrService} from "./toastr.service";
import {ApiService} from "./api.service";
import {ReportsService} from "./reports/reports.service";
import {EveboxSubscriptionService} from "./subscription.service";
import {
    SshReportComponent,
    SshTopClientsComponent, SshTopServersComponent, IpAddrDataTableComponent
} from "./reports/ssh-report.component";

@NgModule({
    declarations: [
        AppComponent,

        AlertsComponent,
        EventsComponent,
        EventComponent,

        // Report components.
        DNSReportComponent,
        AlertReportComponent,
        NetflowReportComponent,
        FlowReportComponent,
        EveboxMetricsGraphicComponent,
        EveboxReportDataTable,
        IpReportComponent,
        SshReportComponent,
        SshTopClientsComponent,
        SshTopServersComponent,
        IpAddrDataTableComponent,

        EveboxFilterInputComponent,

        TopNavComponent,
        EveboxHelpComponent,
        AceEditor,
        AlertTableComponent,
        EveboxEventTableComponent,
        KeyTableDirective,
        EveboxDurationComponent,
        EveboxSearchLinkComponent,
        EveboxEventTable2Component,
        EveboxLoadingSpinnerComponent,

        EveboxFormatIpAddressPipe,
        EveboxHumanizePipe,
        EveboxJsonPrettyPipe,
        EveboxMapToItemsPipe,
        EveBoxGenericPrettyPrinter,
        EveboxBase64DecodePipe,
        EveboxHexPipe,
        EveBoxEventDescriptionPrinterPipe,
        EventSeverityToBootstrapClass,
        EveboxFormatTimestampPipe,
    ],
    imports: [
        // Angular modules.
        BrowserModule,
        FormsModule,
        HttpModule,
        RouterModule,

        // Evebox modules.
        routing,
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
        EveboxFormatIpAddressPipe,
    ],
    bootstrap: [AppComponent]
})
export class AppModule {
}
