import {Component, OnInit, Input} from "@angular/core";
import {ReportsService} from "./reports.service";
import {EveboxSearchLinkComponent} from "./search-link.component";
import {ROUTER_DIRECTIVES, Router} from "@angular/router";
import {AppService, AppEventCode} from "./app.service";

@Component({
    selector: "simple-report",
    template: `<div class="panel panel-default">
  <div class="panel-heading">
    <b>{{title}}</b>
  </div>
  <div *ngIf="!rows">
    <div class="panel-body" style="text-align: center;">
      <i class="fa fa-spinner fa-pulse"
         style="font-size: 200px; opacity: 0.5;"></i>
    </div>
  </div>

  <table class="table table-striped table-condensed">
    <thead>
    <tr>
      <th></th>
      <th>#</th>
      <th>{{header}}</th>
    </tr>
    </thead>
    <tbody>
    <tr *ngFor="let row of rows; let i = index">
      <td>{{i + 1}}</td>
      <td>{{row.count}}</td>
      <td>
        <a href="{{searchLink(row)}}">{{row.key}}</a>
      </td>
    </tr>
    </tbody>
  </table>
</div>`,
    directives: [
        EveboxSearchLinkComponent
    ]
})
class SimpleReport implements OnInit {

    @Input() private title:string;
    @Input() private rows:any[];
    @Input() private header:string;
    @Input() private searchField:string;

    constructor(private router:Router) {
    }

    ngOnInit() {
    }

    searchLink(row:any) {
        return "#/alerts?q=" + `+${this.searchField}:"${row.key}"`;
    }
}

@Component({
    template: `<div class="alert alert-warning alert-dismissable" role="alert">
  <button type="button" class="close" data-dismiss="alert" aria-label="Close">
    <span aria-hidden="true">&times;</span></button>
  <b>Note:<b></b> These reports are experimental and are subject to change - for
    the better!</b>
</div>

<div class="row">
  <div class="col-md-12">
    <simple-report title="Top Alert Signatures" [rows]="signatureRows"
                   header="Signature"
                   searchField="alert.signature.raw"></simple-report>
  </div>
</div>

<div class="row">
  <div class="col-md-6">
    <simple-report title="Top Alerting Source IPs" [rows]="sourceRows"
                   searchField="src_ip"
                   header="Source"></simple-report>
  </div>
  <div class="col-md-6">
    <simple-report title="Top Alerting Destination IPs" [rows]="destinationRows"
                   searchField="dest_ip"
                   header="Destination"></simple-report>
  </div>
</div>`,
    directives: [
        SimpleReport,
        EveboxSearchLinkComponent,
        ROUTER_DIRECTIVES
    ]
})
export class ReportsComponent implements OnInit {

    private sourceRows:any[];
    private destinationRows:any[];
    private signatureRows:any[];

    private dispatcherSubscription:any;

    constructor(private appService:AppService,
                private reports:ReportsService) {
    }

    ngOnInit() {

        this.refresh();

        this.dispatcherSubscription = this.appService.subscribe((event:any) => {
            if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
                this.refresh();
            }
        });

    }

    mapAggregation(response:any, name:string):any[] {
        return response.aggregations[name].buckets.map((item:any) => {
            return {
                count: item.doc_count,
                key: item.key
            }
        });
    }

    refresh() {

        this.sourceRows = undefined;
        this.destinationRows = undefined;
        this.signatureRows = undefined;

        this.reports.findAlertsGroupedBySourceIp().then(
            (response:any) => {
                console.log(response);
                this.sourceRows = this.mapAggregation(response, "sources");
                this.destinationRows = this.mapAggregation(response, "destinations");
                this.signatureRows = this.mapAggregation(response, "signatures");
            });
    }
}