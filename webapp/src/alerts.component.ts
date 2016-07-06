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

import {AlertService} from "./alert.service";
import {Component, OnInit, OnDestroy} from "@angular/core";
import {AlertTableComponent} from "./alert-table.component";
import {ElasticSearchService, AlertGroup} from "./elasticsearch.service";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";
import {Router} from "@angular/router";
import {MousetrapService} from "./mousetrap.service";
import {AppService, AppEvent, AppEventCode} from "./app.service";
import {EventService} from "./event.service";
import {ToastrService} from "./toastr.service";

const TEMPLATE:string = `<div [ngClass]="{'evebox-opacity-50': loading}">

  <loading-spinner [loading]="loading"></loading-spinner>

  <!-- Button and filter bar. -->
  <div class="row">
    <div class="col-md-6">
      <button type="button" class="btn btn-default" (click)="refresh()">
        Refresh
      </button>
      <button *ngIf="rows.length > 0 && !allSelected()" type="button"
              class="btn btn-default"
              (click)="selectAllRows()">Select All
      </button>
      <button *ngIf="rows.length > 0 && allSelected()" type="button"
              class="btn btn-default"
              (click)="deselectAllRows()">Deselect All
      </button>
      <button *ngIf="rows.length > 0 && getSelectedCount() > 0"
              type="button"
              class="btn btn-default"
              (click)="archiveSelected()">Archive
      </button>
      <button *ngIf="rows.length > 0 && getSelectedCount() > 0"
              type="button"
              class="btn btn-default"
              (click)="escalateSelected()">Escalate
      </button>
    </div>
    <div class="col-md-6">

      <br class="hidden-lg hidden-md"/>

      <form (submit)="submitFilter()">
        <div class="input-group">
          <input id="filter-input" type="text" class="form-control"
                 placeholder="Filter..." [(ngModel)]="queryString"/>
          <div class="input-group-btn">
            <button class="btn btn-default" type="submit">Apply
            </button>
            <button type="button" class="btn btn-default" (click)="clearFilter()">Clear</button>
          </div>
        </div>
      </form>
    </div>
  </div>

  <div *ngIf="!loading && rows.length == 0" style="text-align: center;">
    <hr/>
    No events found.
    <hr/>
  </div>

  <br/>

  <alert-table
      *ngIf="rows.length > 0"

      (rowClicked)="rowClicked($event)"
      (toggleEscalation)="toggleEscalatedState($event)"
      (archiveEvent)="archiveAlertGroup($event)"
      [(activeRow)]="activeRow"
      [rows]="rows"></alert-table>
</div>`;

const DIRECTIVES:any[] = [
    AlertTableComponent,
    EveboxLoadingSpinnerComponent
];

@Component({
    template: TEMPLATE,
    directives: DIRECTIVES
})
export class AlertsComponent implements OnInit, OnDestroy {

    private rows:any[] = [];
    private activeRow:number = 0;
    private queryString:string = "";
    private loading:boolean = false;
    private filters:any[] = [];
    private dispatcherSubscription:any;
    private routerSubscription:any;

    constructor(private alertService:AlertService,
                private elasticSearchService:ElasticSearchService,
                private router:Router,
                private mousetrap:MousetrapService,
                private appService:AppService,
                private eventService:EventService,
                private toastr:ToastrService) {
    }

    buildState():any {
        let state = {
            rows: this.rows,
            activeRow: this.activeRow,
        };
        return state;
    }

    isInbox() {
        return this.appService.getRoute() == "/inbox";
    }

    ngOnInit():any {

        if (this.appService.getRoute() == "/inbox") {
            this.filters.push({not: {term: {tags: "archived"}}})
        }
        if (this.appService.getRoute() == "/escalated") {
            this.filters.push({term: {tags: "escalated"}})
            this.appService.disableTimeRange();
        }

        // Listen for changes in the route.
        this.routerSubscription = this.router.routerState.queryParams.subscribe((params:any) => {

            this.queryString = params.q || "";

            if (!this.restoreState()) {
                this.refresh();
            }

        });

        this.mousetrap.bind(this, "/", () => this.focusFilterInput());
        this.mousetrap.bind(this, "* a", () => this.selectAllRows());
        this.mousetrap.bind(this, "* n", () => this.deselectAllRows());
        this.mousetrap.bind(this, "r", () => this.refresh());
        this.mousetrap.bind(this, "o", () => this.openActiveEvent());
        this.mousetrap.bind(this, "f8", () => this.archiveActiveEvent());
        this.mousetrap.bind(this, "s", () =>
            this.toggleEscalatedState(this.getActiveRow()));
        this.mousetrap.bind(this, "x", () =>
            this.toggleSelectedState(this.getActiveRow()));
        this.mousetrap.bind(this, "e", () => this.archiveEvents());

        this.dispatcherSubscription = this.appService.subscribe((event:any) => {
            this.appEventHandler(event);
        });
    }

    ngOnDestroy():any {
        this.mousetrap.unbind(this);
        this.routerSubscription.unsubscribe();
    }

    toggleSelectedState(row:any) {
        row.selected = !row.selected;
    }

    restoreState():boolean {

        let state = this.alertService.popState();
        if (!state) {
            return false;
        }

        console.log("Restoring previous state.");

        let rows = state.rows;
        let activeRow = state.activeRow;

        // If in inbox, remove any archived events.
        if (this.isInbox()) {
            rows = rows.filter((row:any) => {
                return row.event.event._source.tags.indexOf("archived") == -1;
            });
            if (activeRow >= rows.length) {
                activeRow = rows.length - 1;
            }
        }

        this.rows = rows;
        this.activeRow = activeRow;

        return true;
    }

    archiveEvents() {
        // If rows are selected, archive the selected rows, otherwise archive
        // the current active event.
        if (this.getSelectedCount() > 0) {
            this.archiveSelected();
        }
        else {
            this.archiveAlertGroup(this.getActiveRow());
        }
    }

    appEventHandler(event:AppEvent) {

        if (event.event == AppEventCode.TIME_RANGE_CHANGED) {
            this.refresh();
        }

    }

    openActiveEvent() {
        this.openEvent(this.getActiveRow().event);
    }

    archiveActiveEvent() {
        this.archiveAlertGroup(this.getActiveRow());
    }

    getActiveRow() {
        return this.rows[this.getActiveRowIndex()];
    }

    getActiveRowIndex() {
        return this.activeRow;
    }

    escalateSelected() {
        let selected = this.rows.filter((row:any) => {
            return row.selected;
        });
        selected.forEach((row:any) => {

            // Optimistically mark as all escalated.
            row.event.escalatedCount = row.event.count;

            this.elasticSearchService.escalateAlertGroup(row.event);
        })
    }

    archiveSelected() {
        let selected = this.rows.filter((row:any) => {
            return row.selected &&
                row.event.event._source.tags.indexOf("archived") < 0;
        });
        selected.forEach((row:any) => {
            this.archiveAlertGroup(row);
        });
    }

    archiveAlertGroup(row:any) {
        // Optimistically mark the event as archived.
        row.event.event._source.tags.push("archived");

        // If in inbox, also remove it from view.
        if (this.appService.getRoute() == "/inbox") {
            this.removeRow(row);
        }

        this.elasticSearchService.archiveAlertGroup(row.event);
    }

    removeRow(row:any) {
        this.rows = this.rows.filter((_row:any) => {
            if (_row == row) {
                return false;
            }
            return true;
        });
        if (this.activeRow >= this.rows.length) {
            this.activeRow--;
        }
    }

    focusFilterInput() {
        document.getElementById("filter-input").focus();
    }

    /**
     * Return true if all rows are selected.
     */
    allSelected() {
        return this.rows.every((row:any) => {
            return row.selected;
        })
    }

    getSelectedRows() {
        return this.rows.filter((row:any) => {
            return row.selected;
        });
    }

    getSelectedCount() {
        return this.getSelectedRows().length;
    }

    selectAllRows() {
        this.rows.forEach((row:any) => {
            row.selected = true;
        });
    }

    deselectAllRows() {
        this.rows.forEach((row:any) => {
            row.selected = false;
        });
    }

    submitFilter() {
        this.appService.updateQueryParameters({q: this.queryString});
        document.getElementById("filter-input").blur();
        this.refresh();
    }

    openEvent(event:AlertGroup) {

        // Save the current state of this.
        this.alertService.pushState(this.buildState());

        this.eventService.pushAlertGroup(event);
        this.router.navigate(['/event', event.event._id]);
    }

    rowClicked(row:any) {
        this.openEvent(row.event);
    }

    clearFilter() {
        this.queryString = "";
        this.submitFilter();
    }

    toggleEscalatedState(row:any, event?:any) {

        if (event) {
            event.stopPropagation();
        }

        let alertGroup:AlertGroup = row.event;

        if (alertGroup.escalatedCount < alertGroup.count) {

            // Optimistically mark as all escalated.
            alertGroup.escalatedCount = alertGroup.count;

            this.elasticSearchService.escalateAlertGroup(alertGroup);
        }

        else if (alertGroup.escalatedCount == alertGroup.count) {

            // Optimistically mark all as de-escalated.
            alertGroup.escalatedCount = 0;

            this.elasticSearchService.removeEscalatedStateFromAlertGroup(alertGroup);
        }
    }

    refresh() {

        // Prevent double loading.
        if (this.loading) {
            return;
        }

        this.loading = true;

        this.alertService.fetchAlerts({
            queryString: this.queryString,
            filters: this.filters
        }).then((rows:any) => {
            this.rows = rows;
        }, (error:any) => {

            console.log("Error fetching alerts:");
            console.log(error);

            // Check for a reason.
            try {
                this.toastr.error(error.error.root_cause[0].reason);
            }
            catch (err) {
                this.toastr.error("An error occurred while executing query.");
            }

            this.rows = [];

        }).then(() => {
            this.loading = false;
        });
    }

}
