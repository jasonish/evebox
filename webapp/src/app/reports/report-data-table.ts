// Copyright (C) 2016-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Component, Input } from "@angular/core";

@Component({
  selector: "report-data-table",
  template: `<div class="card" [ngClass]="{ 'evebox-opacity-50': loading > 0 }">
    <div class="card-header">
      <b>{{ title }}</b>
    </div>
    <div *ngIf="loading > 0 || !rows">
      <i
        class="fa fa-spinner fa-pulse"
        style="position: absolute; left: 50%; margin-left: -100px; font-size: 200px; opacity: 0.5;"
      ></i>
    </div>

    <div *ngIf="!rows || rows.length == 0" class="card-body">No data.</div>

    <table
      *ngIf="rows && rows.length > 0"
      class="table table-sm table-striped table-hover"
    >
      <thead>
        <tr>
          <th>{{ headers[0] }}</th>
          <th>{{ headers[1] }}</th>
        </tr>
      </thead>
      <tbody>
        <tr *ngFor="let row of rows; let i = index">
          <td>{{ row.count }}</td>
          <td>
            <a [routerLink]="['/events', { q: q(row) }]">{{ row.key }}</a>
          </td>
        </tr>
      </tbody>
    </table>
  </div>`,
  styles: [
    "a { text-decoration: none; }",
    "tbody { border-top: 1px solid darkgray !important; }",
    "table { padding-bottom: 0; margin-bottom: 0; }",
  ],
})
export class EveboxReportDataTable {
  @Input() title: string;
  @Input() headers: string[] = [];
  @Input() rows: any[];
  @Input() loading = 0;

  q(row: any) {
    if (row.searchKey) {
      return `+"${row.searchKey}"`;
    }
    return `+"${row.key}"`;
  }
}
