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

import {BrowserModule} from "@angular/platform-browser";
import {NgModule} from "@angular/core";
import {FormsModule} from "@angular/forms";
import {BrowserAnimationsModule} from "@angular/platform-browser/animations";

import {AppComponent} from "./app.component";
import {RouterModule} from "@angular/router";
import {AuthGuard, ConfigResolver, NeverActivate, routing} from "./app.routes";
import {AlertsComponent} from "./alerts.component";
import {EventComponent} from "./event/event.component";
import {EventsComponent} from "./events/events.component";
import {DNSReportComponent} from "./reports/dns-report/dns-report.component";
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
    IpAddrDataTableComponent,
    SshReportComponent
} from "./reports/ssh-report.component";
import {LoginComponent} from "./login/login.component";
import {SettingsComponent} from "./settings/settings.component";
import {ThemeService} from "./shared/theme.service";
import {SettingsService} from "./settings.service";
import {EveboxPrintablePipe} from "./pipes/printable.pipe";
import {RuleHighlightPipe} from "./pipes/rule-highlight.pipe";
import {HttpClientModule} from "@angular/common/http";
import {DebugComponent} from "./debug/debug.component";
import {ClientService} from "./client.service";
import {EveBoxProtoPrettyPrinter} from "./pipes/proto-pretty-printer.pipe";
import { CommentInputComponent } from './comment-input/comment-input.component';

@NgModule({
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
        IpReportComponent,
        SshReportComponent,
        IpAddrDataTableComponent,
        EveboxFilterInputComponent,
        TopNavComponent,
        EveboxHelpComponent,
        AceEditor,
        EveboxEventTableComponent,
        KeyTableDirective,
        EveboxDurationComponent,
        EveboxSearchLinkComponent,
        EveboxEventTable2Component,
        EveboxLoadingSpinnerComponent,
        LoginComponent,
        SettingsComponent,
        DebugComponent,

        // Local pipes.
        EveBoxProtoPrettyPrinter,
        EventSeverityToBootstrapClass,
        EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EveboxHumanizePipe,
        EveboxJsonPrettyPipe,
        EveboxMapToItemsPipe,
        EveBoxGenericPrettyPrinter,
        EveboxBase64DecodePipe,
        EveboxHexPipe,
        EveBoxEventDescriptionPrinterPipe,
        RuleHighlightPipe,
        EveboxPrintablePipe,
        CommentInputComponent,
    ],
    imports: [
        // Angular modules.
        BrowserModule,
        FormsModule,
        RouterModule,
        BrowserAnimationsModule,
        HttpClientModule,

        // Evebox modules.
        routing,
    ],
    providers: [
        AlertService,
        AppService,
        ApiService,
        AuthGuard,
        ConfigService,
        ClientService,
        ConfigResolver,
        ElasticSearchService,
        EventServices,
        EventService,
        EveboxSubscriptionService,
        MousetrapService,
        ReportsService,
        SettingsService,
        TopNavService,
        ToastrService,
        ThemeService,

        // Route gards.
        NeverActivate,

        // Local pipes.
        EveBoxProtoPrettyPrinter,
        EveboxFormatIpAddressPipe,

        // Angular included pipes.
    ],
    bootstrap: [AppComponent]
})
export class AppModule {
}
