/* Copyright (c) 2016 Jason Ish
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

import {Component, Input} from "@angular/core";

@Component({
    selector: "report-data-table",
    template: `<div class="panel panel-default" [ngClass]="{'evebox-opacity-50': loading > 0}">
  <div class="panel-heading">
    <b>{{title}}</b>
  </div>
  <div *ngIf="loading > 0 || !rows">
    <i class="fa fa-spinner fa-pulse"
       style="position: absolute; left: 50%; margin-left: -100px; font-size: 200px; opacity: 0.5;"></i>
  </div>

  <div *ngIf="!rows || rows.length == 0" class="panel-body">
    No data.
  </div>

  <table *ngIf="rows && rows.length > 0"
         class="table table-striped table-condensed">
    <thead>
    <tr>
      <th class="col-md-2">{{headers[0]}}</th>
      <th class="col-md-10">{{headers[1]}}</th>
    </tr>
    </thead>
    <tbody>
    <tr *ngFor="let row of rows; let i = index">
      <td class="col-md-1">{{row.count}}</td>
      <td class="col-md-11">
        <a [routerLink]="['/events', {q: q(row)}]">{{row.key}}</a>
      </td>
    </tr>
    </tbody>
  </table>
</div>`,
})
export class EveboxReportDataTable {

    @Input() private title:string;
    @Input() private headers:string[] = [];
    @Input() private rows:any[];
    @Input() private loading:number = 0;

    q(row:any) {
        if (row.searchKey) {
            return `+"${row.searchKey}"`;
        }
        return `+"${row.key}"`;
    }
}

