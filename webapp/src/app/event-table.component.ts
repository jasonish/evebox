// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Component, Input, OnDestroy, OnInit } from "@angular/core";
import { Router } from "@angular/router";
import { MousetrapService } from "./mousetrap.service";
import { ElasticSearchService } from "./elasticsearch.service";
import { indexOf } from "./utils";

@Component({
  selector: "evebox-event-table",
  template: ` <div *ngIf="rows && rows.length > 0">
    <table
      class="evebox-event-table"
      eveboxKeyTable
      [rows]="rows"
      [(activeRow)]="activeRow"
    >
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
      <tbody>
        <tr
          *ngFor="let row of rows; let i = index"
          [ngClass]="
            row | eventSeverityToBootstrapClass: 'evebox-bg-':'success'
          "
          (click)="openRow(row)"
        >
          <td>
            <i *ngIf="i == activeRow" class="fa fa-chevron-right"></i>
          </td>
          <td class="text-nowrap">
            {{ row._source.timestamp | eveboxFormatTimestamp }}
            <br />
            <evebox-duration
              style="color: gray"
              [timestamp]="row._source.timestamp"
            ></evebox-duration>
          </td>
          <td>{{ row._source.event_type | uppercase }}</td>
          <td class="text-nowrap">
            <div *ngIf="row._source.src_ip || row._source.dest_ip">
              <label>S:</label>
              {{ row._source.src_ip | eveboxFormatIpAddress }}
              <br />
              <label>D:</label>
              {{ row._source.dest_ip | eveboxFormatIpAddress }}
            </div>
          </td>
          <td style="word-break: break-all;">
            {{ row | eveboxEventDescriptionPrinter }}
            <span
              class="badge bg-secondary"
              *ngIf="
                row._source.app_proto && row._source.app_proto !== 'failed'
              "
            >
              {{ row._source.app_proto }}
            </span>
            <div
              *ngIf="getEventType(row) === 'alert' && !isArchived(row)"
              class="pull-right"
              (click)="$event.stopPropagation()"
            >
              <button
                type="button"
                class="btn btn-secondary"
                (click)="archive(row, $event)"
              >
                Archive
              </button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>`,
})
export class EveboxEventTableComponent implements OnInit, OnDestroy {
  @Input() rows: any[] = null;

  activeRow = 0;

  constructor(
    private router: Router,
    private mousetrap: MousetrapService,
    private elasticSearchService: ElasticSearchService
  ) {}

  ngOnInit() {
    this.mousetrap.bind(this, "o", () => {
      this.openActiveRow();
    });
  }

  ngOnDestroy() {
    this.mousetrap.unbind(this);
  }

  getActiveRow() {
    let row = this.rows[this.activeRow];
    return row;
  }

  openActiveRow() {
    this.openRow(this.getActiveRow());
  }

  openRow(row: any) {
    this.router.navigate(["/event", row._id]);
  }

  getEventType(row: any) {
    return row._source.event_type;
  }

  isArchived(row: any): boolean {
    try {
      if (indexOf(row._source.tags, "evebox.archived") > -1) {
        return true;
      }
      if (indexOf(row._source.tags, "archived") > -1) {
        return true;
      }
    } finally {
    }
    return false;
  }

  archive(row: any, $event?: any) {
    if ($event) {
      $event.stopPropagation();
    }
    this.elasticSearchService.archiveEvent(row).then((response: any) => {
      if (!row._source.tags) {
        row._source.tags = [];
      }
      row._source.tags.push("archived");
      row._source.tags.push("evebox.archived");
    });
  }
}
