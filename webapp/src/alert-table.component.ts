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

import {
    Component,
    Input,
    Output,
    EventEmitter,
    OnInit,
    AfterViewChecked,
    OnDestroy
} from "@angular/core";
import {AppService} from "./app.service";
import {MousetrapService} from "./mousetrap.service";
import {ActivatedRoute} from "@angular/router";

declare var $:any;

@Component({
    selector: "alert-table",
    template: `<div class="table-responsive" style="overflow: inherit !important;">

  <table class="table table-condensed table-hover evebox-event-table"
         eveboxKeyTable [rows]="rows" [(activeRow)]="activeRow"
         (activeRowChange)="activeRowChange.emit($event)">

    <thead>
    <tr>
      <th></th>
      <th></th>
      <th></th>
      <th>#</th>
      <th>Timestamp</th>
      <th>Source/Dest</th>
      <th width="50%">Signature</th>
    </tr>
    </thead>

    <tbody>
    <tr *ngFor="let row of rows; let i = index"
        [ngClass]="row.event.event | eventSeverityToBootstrapClass:'evebox-'"
        (click)="onRowClick($event, row)">
      <td style="width: 1% !important;">
      <span *ngIf="i == activeRow"
            class="glyphicon glyphicon-chevron-right"></span>
      </td>
      <td class="clearfix" style="width: 1% !important;">
        <input type="checkbox" [(ngModel)]="row.selected">
      </td>
      <td (click)="toggleEscalation.emit(row)"
          style="width: 1% !important;">
        <i *ngIf="row.event.escalatedCount == 0"
           class="fa fa-star-o"></i>
        <i *ngIf="row.event.escalatedCount == row.event.count"
           class="fa fa-star"></i>
        <i *ngIf="row.event.escalatedCount > 0 &&  row.event.escalatedCount != row.event.count"
           class="fa fa-star-half-o"></i>
      </td>
      <td style="width: 1% !important;">{{row.event.count}}</td>
      <td class="text-nowrap" style="width: 1%;">
        {{row.date | eveboxFormatTimestamp}}
        <br/>
        <evebox-duration style="color:gray"
                         [timestamp]="row.event.newestTs"></evebox-duration>
      </td>
      <td class="text-nowrap" style="width: 1% !important">
        <label>S:</label>
        {{row.event.event._source.src_ip | eveboxFormatIpAddress}}
        <br/>
        <label>D:</label>
        {{row.event.event._source.dest_ip | eveboxFormatIpAddress}}
      </td>
      <td>
        <div *ngIf="!isArchived(row)" class="btn-group pull-right">
          <button type="button"
                  class="btn btn-default"
                  (click)="archiveEvent.emit(row)">
            Archive
          </button>
          <button type="button" class="btn btn-default"
                  title="Escalate and Archive"
                  (click)="escalateAndArchiveEvent.emit(row);">
              <!-- This is supposed to be a star with an archive box overlaid,
                 the idea behing to escalate and archive the event. -->
              <i class="fa fa-star-o fa-lg"></i>
              <i class="fa fa-archive" style="position: absolute; left: 50%; top: 40%"></i>
          </button>
          <button type="button" id="dropdown-{{i}}"
                  class="btn btn-default dropdown-toggle"
                  data-toggle="dropdown" aria-haspopup="true"
                  aria-expanded="false"><span
              class="caret"></span></button>
          <ul class="dropdown-menu">
            <li><a href="javascript:void(0);"
                   (click)="selectBySignatureId(row)">1. Select
              all with SID: {{row.event.event._source.alert.signature_id}}</a>
            </li>
            <li><a href="javascript:void(0)" (click)="filterBySignatureId(row)">2.
              Filter
              on SID: {{row.event.event._source.alert.signature_id}}</a></li>
          </ul>
        </div>
        <span *ngIf="row.event.event._source.alert.action != 'allowed'"
              class="label label-warning">{{row.event.event._source.alert.action | uppercase}}</span>
        <span
            [innerHTML]="row.event.event | eveboxEventDescriptionPrinter"></span>
      </td>
    </tr>
    </tbody>

  </table>

</div>`,
})
export class AlertTableComponent implements OnInit, OnDestroy, AfterViewChecked {

    @Input() private rows:any[] = [];
    @Output() private rowClicked:EventEmitter<any> = new EventEmitter<any>();
    @Input() activeRow:number = 0;
    @Output() activeRowChange:EventEmitter<number> = new EventEmitter<number>();
    @Output() toggleEscalation:EventEmitter<any> = new EventEmitter<any>();
    @Output() archiveEvent:EventEmitter<any> = new EventEmitter<any>();

    @Output() escalateAndArchiveEvent:EventEmitter<any> = new EventEmitter<any>();

    constructor(private appService:AppService,
                private mousetrap:MousetrapService,
                private route:ActivatedRoute) {
    }

    ngOnInit() {
        this.mousetrap.bind(this, ".", () => {
            this.openDropdownMenu();
        });

        this.mousetrap.bind(this, "1", () => {
            this.selectBySignatureId(this.rows[this.activeRow]);
        });
        this.mousetrap.bind(this, "2", () => {
            this.filterBySignatureId(this.rows[this.activeRow]);
        });
    }

    onRowClick($event:any, row:any) {
        if ($event.srcElement.parentElement == $event.currentTarget) {
            this.rowClicked.emit(row);
        }
    }

    ngOnDestroy() {
        this.mousetrap.unbind(this);
    }

    ngAfterViewChecked() {
        $(".dropdown-toggle").dropdown();
    }

    openDropdownMenu() {
        // Toggle.
        let element = $("#dropdown-" + this.activeRow);
        element.dropdown('toggle');

        // Focus.
        element.find("li:first-child a").focus();
    }

    isArchived(row:any) {
        if (row.event.event._source.tags) {
            if (row.event.event._source.tags.indexOf("archived") > -1) {
                return true;
            }
        }
        return false;
    }

    selectBySignatureId(row:any) {

        let signatureId = row.event.event._source.alert.signature_id;

        this.rows.forEach((row:any) => {
            if (row.event.event._source.alert.signature_id === signatureId)
                row.selected = true;
        });

        // Probably a little broad but gets the job done.
        $("table .open").dropdown('toggle');
    }

    filterBySignatureId(row:any) {

        // Probably a little broad but gets the job done.
        $(".open").dropdown('toggle');

        let signatureId = row.event.event._source.alert.signature_id;
        this.appService.updateParams(this.route, {
            q: `alert.signature_id:${signatureId}`
        });

    }
}