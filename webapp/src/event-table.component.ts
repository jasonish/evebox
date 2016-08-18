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

import {Component, Input, OnInit, OnDestroy} from "@angular/core";
import {Router} from "@angular/router";
import {EveboxFormatTimestampPipe} from "./pipes/format-timestamp.pipe";
import {EveboxFormatIpAddressPipe} from "./pipes/format-ipaddress.pipe";
import {KeyTableDirective} from "./keytable.directive";
import {EveBoxEventDescriptionPrinterPipe} from "./pipes/eventdescription.pipe";
import {EveboxDurationComponent} from "./duration.component";
import {EventSeverityToBootstrapClass} from "./pipes/event-severity-to-bootstrap-class.pipe";
import {MousetrapService} from "./mousetrap.service";
import {ElasticSearchService} from "./elasticsearch.service";

export interface EveboxEventTableConfig {
    showCount:boolean,
    rows:any[]
}

@Component({
    selector: "eveboxEventTable",
    template: `<div *ngIf="config.rows && config.rows.length > 0" class="table-responsive">
  <table class="table table-condensed table-hover evebox-event-table"
         eveboxKeyTable [rows]="config.rows" [(activeRow)]="activeRow">
    <thead>
    <tr>
      <!-- Chevron column. -->
      <th></th>
      <!-- Timestamp. -->
      <th>Timestamp</th>
      <!-- Event type. -->
      <th>Type</th>
      <!-- Source/Dest. -->
      <th>Source/Dest</th>
      <!-- Description. -->
      <th>Description</th>
    </tr>
    </thead>
    <tbody *ngIf="config.rows.length > 0">
    <tr *ngFor="let row of config.rows; let i = index"
        [ngClass]="row | eventSeverityToBootstrapClass:'':'success'"
        (click)="openRow(row)">
      <td>
        <div *ngIf="i == activeRow"
             class="glyphicon glyphicon-chevron-right"></div>
      </td>
      <td class="text-nowrap">
        {{row._source.timestamp | eveboxFormatTimestamp}}
        <br/>
        <evebox-duration style="color: gray"
                         [timestamp]="row._source.timestamp"></evebox-duration>
      </td>
      <td>{{row._source.event_type | uppercase}}</td>
      <td class="text-nowrap">
        <label>S:</label>
        {{row._source.src_ip | eveboxFormatIpAddress}}
        <br/>
        <label>D:</label>
        {{row._source.dest_ip | eveboxFormatIpAddress}}
      </td>
      <td style="word-break: break-all;">{{row |
        eveboxEventDescriptionPrinter}}
        <div *ngIf="getEventType(row) == 'alert' && ! isArchived(row)"
             class="pull-right"
             (click)="$event.stopPropagation()">
          <button type="button" class="btn btn-default"
                  (click)="archive(row, $event)">Archive
          </button>
        </div>
      </td>
    </tr>
    </tbody>
  </table>
</div>`,
    pipes: [EveboxFormatTimestampPipe,
        EveboxFormatIpAddressPipe,
        EveBoxEventDescriptionPrinterPipe,
        EventSeverityToBootstrapClass
    ],
    directives: [
        KeyTableDirective,
        EveboxDurationComponent
    ]
})
export class EveboxEventTableComponent implements OnInit, OnDestroy {

    @Input() private config:EveboxEventTableConfig;
    private activeRow:number = 0;

    constructor(private router:Router,
                private mousetrap:MousetrapService,
                private elasticSearchService:ElasticSearchService) {
    }

    ngOnInit() {
        this.mousetrap.bind(this, "o", ()=> {
            this.openActiveRow();
        });
    }

    ngOnDestroy() {
        this.mousetrap.unbind(this);
    }

    getActiveRow() {
        let row = this.config.rows[this.activeRow];
        return row;
    }

    openActiveRow() {
        this.openRow(this.getActiveRow());
    }

    openRow(row:any) {
        this.router.navigate(['/event', row._id]);
    }

    getEventType(row:any) {
        return row._source.event_type;
    }

    isArchived(row:any) {
        try {
            return row._source.tags.indexOf("archived") > -1;
        }
        catch (e) {
            return false;
        }
    }

    archive(row:any, $event?:any) {
        if ($event) {
            $event.stopPropagation();
        }
        this.elasticSearchService.archiveEvent(row);
    }
}