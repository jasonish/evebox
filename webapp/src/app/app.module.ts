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
        BrowserModule,
        FormsModule,
        HttpModule,
        RouterModule,
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
