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

import {Component, Input, Output, EventEmitter} from "@angular/core";
import {EveboxDurationComponent} from "./duration.component";
import {EveboxFormatTimestampPipe} from "./pipes/format-timestamp.pipe";
import {EveboxFormatIpAddressPipe} from "./pipes/format-ipaddress.pipe";
import {EventSeverityToBootstrapClass} from "./pipes/event-severity-to-bootstrap-class.pipe";
import {KeyTableDirective} from "./keytable.directive";
import {EveBoxEventDescriptionPrinterPipe} from "./pipes/eventdescription.pipe";

@Component({
    selector: "alert-table",
    template: `<div class="table-responsive">

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
        [ngClass]="row.event.event | eventSeverityToBootstrapClass"
        (click)="rowClicked.emit(row)">
      <td>
      <span *ngIf="i == activeRow"
            class="glyphicon glyphicon-chevron-right"></span>
      </td>
      <td>
        <input type="checkbox" [(ngModel)]="row.selected" (click)="$event.stopPropagation()">
      </td>
      <td (click)="$event.stopPropagation(); toggleEscalation.emit(row)">
        <i *ngIf="row.event.escalatedCount == 0"
           class="fa fa-star-o"></i>
        <i *ngIf="row.event.escalatedCount == row.event.count"
           class="fa fa-star"></i>
        <i *ngIf="row.event.escalatedCount > 0 &&  row.event.escalatedCount != row.event.count"
           class="fa fa-star-half-o"></i>
      </td>
      <td>{{row.event.count}}</td>
      <td class="text-nowrap">
        {{row.date | eveboxFormatTimestamp}}
        <br/>
        <evebox-duration style="color:gray"
                         [timestamp]="row.event.newestTs"></evebox-duration>
      </td>
      <td class="text-nowrap">
        <label>S:</label>
        {{row.event.event._source.src_ip | eveboxFormatIpAddress}}
        <br/>
        <label>D:</label>
        {{row.event.event._source.dest_ip | eveboxFormatIpAddress}}
      </td>
      <td>
        <button *ngIf="!isArchived(row)" type="button"
                class="btn btn-default pull-right"
                (click)="$event.stopPropagation(); archiveEvent.emit(row)">Archive
        </button>
        <span [innerHTML]="row.event.event | eveboxEventDescriptionPrinter"></span>
      </td>
    </tr>
    </tbody>

  </table>

</div>`,
    pipes: [
        EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EventSeverityToBootstrapClass,
        EveBoxEventDescriptionPrinterPipe
    ],
    directives: [
        EveboxDurationComponent,
        KeyTableDirective
    ]
})
export class AlertTableComponent {

    @Input() private rows:any[] = [];
    @Output() private rowClicked:EventEmitter<any> = new EventEmitter<any>();
    @Input() activeRow:number = 0;
    @Output() activeRowChange:EventEmitter<number> = new EventEmitter<number>();
    @Output() toggleEscalation:EventEmitter<any> = new EventEmitter<any>();
    @Output() archiveEvent:EventEmitter<any> = new EventEmitter<any>();

    isArchived(row:any) {
        if (row.event.event._source.tags) {
            if (row.event.event._source.tags.indexOf("archived") > -1) {
                return true;
            }
        }
        return false;
    }

}