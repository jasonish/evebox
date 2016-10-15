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
import {ElasticSearchService, AlertGroup} from "./elasticsearch.service";
import {Router, ActivatedRoute} from "@angular/router";
import {MousetrapService} from "./mousetrap.service";
import {AppService, AppEvent, AppEventCode} from "./app.service";
import {EventService} from "./event.service";
import {ToastrService} from "./toastr.service";
import {TopNavService} from "./topnav.service";
import {EveboxSubscriptionService} from "./subscription.service";
import {loadingAnimation} from "./animations";

declare var window:any;

export interface AlertsState {
    rows:any[];
    activeRow:number;
    route:string,
    queryString:string;
    scrollOffset:number;
}

@Component({
    template: `
<div [@loadingState]="!rows || (!silentRefresh && loading) ? 'true' : 'false'">

  <loading-spinner [loading]="!rows || !silentRefresh && loading"></loading-spinner>

  <!-- Button and filter bar. -->
  <div class="row">
    <div class="col-md-6">
      <button type="button" class="btn btn-default" (click)="refresh()">
        Refresh
      </button>
      <button *ngIf="rows && rows.length > 0 && !allSelected()" type="button"
              class="btn btn-default"
              (click)="selectAllRows()">Select All
      </button>
      <button *ngIf="rows && rows.length > 0 && allSelected()" type="button"
              class="btn btn-default"
              (click)="deselectAllRows()">Deselect All
      </button>
      <button *ngIf="rows && rows.length > 0 && getSelectedCount() > 0"
              type="button"
              class="btn btn-default"
              (click)="archiveSelected()">Archive
      </button>
      <button *ngIf="rows && rows.length > 0 && getSelectedCount() > 0"
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
                 placeholder="Filter..." [(ngModel)]="queryString" name="queryString"/>
          <div class="input-group-btn">
            <button class="btn btn-default" type="submit">Apply
            </button>
            <button type="button" class="btn btn-default" (click)="clearFilter()">Clear</button>
          </div>
        </div>
      </form>
    </div>
  </div>

  <div *ngIf="rows && rows.length == 0" style="text-align: center;">
    <hr/>
    No events found.
    <hr/>
  </div>

  <br/>

  <alert-table
      *ngIf="rows && rows.length > 0"

      (rowClicked)="rowClicked($event)"
      (toggleEscalation)="toggleEscalatedState($event)"
      (archiveEvent)="archiveAlertGroup($event)"
      (escalateAndArchiveEvent)="escalateAndArchiveEvent($event)"
      [(activeRow)]="activeRow"
      [rows]="rows"></alert-table>
</div>`,
    animations: [
        loadingAnimation,
    ]
})
export class AlertsComponent implements OnInit, OnDestroy {

    private rows:any[];
    private activeRow:number = 0;
    private queryString:string = "";
    private loading:boolean = false;
    private dispatcherSubscription:any;

    private silentRefresh:boolean = false;

    constructor(private alertService:AlertService,
                private elasticSearchService:ElasticSearchService,
                private router:Router,
                private route:ActivatedRoute,
                private mousetrap:MousetrapService,
                private appService:AppService,
                private eventService:EventService,
                private toastr:ToastrService,
                private ss:EveboxSubscriptionService,
                private topNavService:TopNavService) {
    }

    buildState():any {
        let state:AlertsState = {
            rows: this.rows,
            activeRow: this.activeRow,
            queryString: this.queryString,
            route: this.appService.getRoute(),
            scrollOffset: window.pageYOffset,
        };
        return state;
    }

    isInbox() {
        return this.appService.getRoute() == "/inbox";
    }

    ngOnInit():any {

        this.ss.subscribe(this, this.route.params, (params:any) => {
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

        // Escalate then archive event.
        this.mousetrap.bind(this, "f9", () => {
            this.escalateAndArchiveEvent(this.getActiveRow());
        });

        this.mousetrap.bind(this, "x", () =>
            this.toggleSelectedState(this.getActiveRow()));
        this.mousetrap.bind(this, "e", () => this.archiveEvents());

        this.dispatcherSubscription = this.appService.subscribe((event:any) => {
            this.appEventHandler(event);
        });
    }

    escalateAndArchiveEvent(row:any) {
        this.archiveAlertGroup(row).then(() => {
            this.toggleEscalatedState(row);
        });
    }

    ngOnDestroy():any {
        this.mousetrap.unbind(this);
        this.ss.unsubscribe(this);
        this.dispatcherSubscription.unsubscribe();
    }

    restoreState():boolean {

        let state:AlertsState = this.alertService.popState();
        if (!state) {
            return false;
        }

        console.log("Restoring previous state.");

        let rows = state.rows;
        let activeRow = state.activeRow;

        if (state.route != this.appService.getRoute()) {
            console.log("Saved state route differs.");
            return false;
        }
        if (state.queryString != this.queryString) {
            console.log("Query strings differ, previous state not being restored.");
            return false;
        }

        // If in inbox, remove any archived events.
        if (this.isInbox()) {
            rows = rows.filter((row:any) => {
                return row.event.event._source.tags.indexOf("archived") == -1;
            });
            if (activeRow >= rows.length) {
                activeRow = rows.length - 1;
            }
        }
        else if (this.appService.getRoute() == "/escalated") {
            rows = rows.filter((row:any) => {
                return row.event.escalatedCount > 0;
            });
            if (activeRow >= rows.length) {
                activeRow = rows.length - 1;
            }
        }

        this.rows = rows;
        this.activeRow = activeRow;

        setTimeout(() => {
            window.scrollTo(0, state.scrollOffset)
        }, 0);

        return true;
    }

    appEventHandler(event:AppEvent) {

        switch (event.event) {
            case AppEventCode.TIME_RANGE_CHANGED:
                this.refresh();
                break;
            case AppEventCode.IDLE:
                if (this.loading) {
                    return;
                }

                if (this.rows.length > 0 && event.data < 60) {
                    return;
                }

                if (this.rows.length == 0 && event.data < 5) {
                    return;
                }

                // Don't auto-refresh if Elastic Search jobs are in progress,
                // could result in reloading events waiting to be archived.
                // TODO: Limit to archive jobs only.
                if (this.elasticSearchService.jobSize() > 0) {
                    console.log("Elastic Search jobs active, not refreshing.");
                    return;
                }

                if (this.rows.length > 0 && this.getSelectedRows().length > 0) {
                    return;
                }

                this.silentRefresh = true;
                this.refresh().then(() => {
                    this.silentRefresh = false;
                });

                break;
        }

    }

    openActiveEvent() {
        this.openEvent(this.getActiveRow().event);
    }

    archiveActiveEvent() {
        if (this.getActiveRowIndex() >= 0) {
            this.archiveAlertGroup(this.getActiveRow());
        }
    }

    getActiveRow() {
        return this.rows[this.getActiveRowIndex()];
    }

    getActiveRowIndex() {
        return this.activeRow;
    }

    toggleSelectedState(row:any) {
        row.selected = !row.selected;
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

        if (!row) {
            return;
        }

        // Optimistically mark the event as archived.
        row.event.event._source.tags.push("archived");

        // If in inbox, also remove it from view.
        if (this.appService.getRoute() == "/inbox") {
            this.removeRow(row);
        }

        return this.elasticSearchService.archiveAlertGroup(row.event);
    }

    archiveEvents() {
        // If rows are selected, archive the selected rows, otherwise archive
        // the current active event.
        if (this.getSelectedCount() > 0) {
            this.archiveSelected();
        }
        else if (this.getActiveRowIndex() > -1) {
            this.archiveAlertGroup(this.getActiveRow());
        }
    }

    removeRow(row:any) {
        console.log("Removing row.")
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
        //this.appService.updateQueryParameters({q: this.queryString});
        this.appService.updateParams(this.route, {q: this.queryString});
        document.getElementById("filter-input").blur();
        this.refresh();
    }

    openEvent(event:AlertGroup) {

        // Save the current state of this.
        this.alertService.pushState(this.buildState());

        this.eventService.pushAlertGroup(event);
        this.router.navigate(['/event', event.event._id, {
            referer: this.appService.getRoute()
        }])
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

        this.loading = true;

        let filters:any[] = [];

        // Add filters depending on view.
        switch (this.appService.getRoute()) {
            case "/inbox":
                // Limit to non-archived events.
                filters.push({not: {term: {tags: "archived"}}});
                break;
            case "/escalated":
                // Limit to escalated events only, no time range applied.
                filters.push({term: {tags: "escalated"}});
                break;
            default:
                break;
        }

        let range:number = 0;

        // Set a time range on all but escalated.
        switch (this.appService.getRoute()) {
            case "/escalated":
                break;
            default:
                range = this.topNavService.getTimeRangeAsSeconds();
                break;
        }

        return this.alertService.fetchAlerts({
            queryString: this.queryString,
            range: range,
            filters: filters
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
            this.activeRow = 0;
            this.loading = false;
            this.appService.resetIdleTime();
        });
    }

}
