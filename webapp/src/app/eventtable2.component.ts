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

import {Component, Input, OnInit, OnDestroy} from '@angular/core';
import {Router} from '@angular/router';
import {EveboxFormatTimestampPipe} from './pipes/format-timestamp.pipe';
import {EveboxFormatIpAddressPipe} from './pipes/format-ipaddress.pipe';
import {KeyTableDirective} from './keytable.directive';
import {EveBoxEventDescriptionPrinterPipe} from './pipes/eventdescription.pipe';
import {EveboxDurationComponent} from './duration.component';
import {EventSeverityToBootstrapClass} from './pipes/event-severity-to-bootstrap-class.pipe';
import {MousetrapService} from './mousetrap.service';
import {ElasticSearchService} from './elasticsearch.service';

@Component({
    selector: 'eveboxEventTable2',
    template: `<div *ngIf="rows && rows.length > 0" class="table-responsive">
  <table class="table table-condensed table-hover evebox-event-table"
         style="padding-bottom: 0px !important; margin-bottom: 0px !important;">
    <thead>
    <tr>
      <!-- Chevron column. -->
      <th></th>
      <!-- Timestamp. -->
      <th>Timestamp</th>
      <!-- Event type. -->
      <th *ngIf="showEventType">Type</th>
      <!-- Source/Dest. -->
      <th>Source/Dest</th>
      <!-- Description. -->
      <th>Description</th>
    </tr>
    </thead>
    <tbody *ngIf="rows.length > 0">
    <tr *ngFor="let row of rows; let i = index"
        [ngClass]="row | eventSeverityToBootstrapClass:'evebox-bg-':'success'"
        (click)="openRow(row)">
      <td>
        <div *ngIf="showActiveEvent && i == activeRow"
             class="glyphicon glyphicon-chevron-right"></div>
      </td>
      <td class="text-nowrap">
        {{row._source.timestamp | eveboxFormatTimestamp}}
        <br/>
        <evebox-duration style="color: gray"
                         [timestamp]="row._source.timestamp"></evebox-duration>
      </td>
      <td *ngIf="showEventType">{{row._source.event_type | uppercase}}</td>
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
})
export class EveboxEventTable2Component {

    @Input() rows: any[];

    @Input() showEventType = true;

    @Input() showActiveEvent = true;

    constructor(private router: Router,
                private mousetrap: MousetrapService,
                private elasticSearchService: ElasticSearchService) {
    }

    openRow(row: any) {
        this.router.navigate(['/event', row._id]);
    }

    getEventType(row: any) {
        return row._source.event_type;
    }

    isArchived(row: any) {
        try {
            return row._source.tags.indexOf('archived') > -1;
        }
        catch (e) {
            return false;
        }
    }

    archive(row: any, $event?: any) {
        if ($event) {
            $event.stopPropagation();
        }
        this.elasticSearchService.archiveEvent(row);
    }
}