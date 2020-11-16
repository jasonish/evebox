// Copyright (C) 2014-2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

import { AlertService } from "./alert.service";
import { AfterViewChecked, Component, OnDestroy, OnInit } from "@angular/core";
import { AlertGroup, ElasticSearchService } from "./elasticsearch.service";
import { ActivatedRoute, Params, Router } from "@angular/router";
import { MousetrapService } from "./mousetrap.service";
import { AppEvent, AppEventCode, AppService } from "./app.service";
import { EventService } from "./event.service";
import { ToastrService } from "./toastr.service";
import { TopNavService } from "./topnav.service";
import { loadingAnimation } from "./animations";
import { SETTING_ALERTS_PER_PAGE, SettingsService } from "./settings.service";
import { debounce } from "rxjs/operators";
import { combineLatest, interval } from "rxjs";

declare var window: any;
declare var $: any;

export interface AlertsState {
    rows: any[];
    allRows: any[];
    activeRow: number;
    route: string;
    queryString: string;
    scrollOffset: number;
}

function compare(a: any, b: any): any {
    const c = {};

    for (const key in b) {
        if (a[key] != b[key]) {
            c[key] = b[key];
        }
    }

    return c;
}

const DEFAULT_SORT_ORDER = "desc";
const DEFAULT_SORT_BY = "timestamp";

@Component({
    templateUrl: "./alerts.component.html",
    animations: [
        loadingAnimation,
    ]
})
export class AlertsComponent implements OnInit, OnDestroy, AfterViewChecked {

    windowSize = 100;
    offset = 0;

    rows: any[] = [];
    allRows: any[] = [];

    activeRow = 0;
    queryString = "";
    loading = false;
    dispatcherSubscription: any;

    silentRefresh = false;

    sortBy: string = DEFAULT_SORT_BY;
    sortOrder: string = DEFAULT_SORT_ORDER;

    constructor(private alertService: AlertService,
                private elasticSearchService: ElasticSearchService,
                private router: Router,
                private route: ActivatedRoute,
                private mousetrap: MousetrapService,
                private appService: AppService,
                private eventService: EventService,
                private toastr: ToastrService,
                private topNavService: TopNavService,
                private settings: SettingsService) {
    }

    ngOnInit(): any {
        this.windowSize = this.settings.getInt(SETTING_ALERTS_PER_PAGE, 100);

        combineLatest([
            this.route.queryParams,
            this.route.params,
        ]).pipe(debounce(() => interval(100))).subscribe(([queryParams, params]) => {
            console.log("Got params and query params...");
            if (params.sortBy) {
                this.sortBy = params.sortBy;
            } else {
                this.sortBy = DEFAULT_SORT_BY;
            }

            if (params.sortOrder) {
                this.sortOrder = params.sortOrder;
            } else {
                this.sortOrder = DEFAULT_SORT_ORDER;
            }

            this.queryString = queryParams.q || "";

            if (!this.restoreState()) {
                this.refresh();
            }
        });

        this.mousetrap.bind(this, "/", () => this.focusFilterInput());
        this.mousetrap.bind(this, "r", () => this.refresh());
        this.mousetrap.bind(this, "o", () => this.openActiveEvent());
        this.mousetrap.bind(this, "f8", () => this.archiveActiveEvent());
        this.mousetrap.bind(this, "s", () =>
            this.toggleEscalatedState(this.getActiveRow()));

        this.mousetrap.bind(this, "* a", () => this.selectAllRows());
        this.mousetrap.bind(this, "* n", () => this.deselectAllRows());
        this.mousetrap.bind(this, "* 1", () => {
            this.selectBySignatureId(this.rows[this.activeRow]);
        });

        // Escalate then archive event.
        this.mousetrap.bind(this, "f9", () => {
            this.escalateAndArchiveEvent(this.getActiveRow());
        });

        this.mousetrap.bind(this, "x", () =>
            this.toggleSelectedState(this.getActiveRow()));
        this.mousetrap.bind(this, "e", () => this.archiveEvents());

        this.mousetrap.bind(this, ">", () => {
            this.older();
        });

        this.mousetrap.bind(this, "<", () => {
            this.newer();
        });

        // CTRL >
        this.mousetrap.bind(this, "ctrl+shift+.", () => {
            this.oldest();
        });

        // CTRL <
        this.mousetrap.bind(this, "ctrl+shift+,", () => {
            this.newest();
        });

        this.dispatcherSubscription = this.appService.subscribe((event: any) => {
            this.appEventHandler(event);
        });

        // Bind "." to open the dropdown menu for the specific event.
        this.mousetrap.bind(this, ".", () => {
            this.openDropdownMenu();
        });
    }

    ngOnDestroy(): any {
        this.mousetrap.unbind(this);
        this.dispatcherSubscription.unsubscribe();
    }

    ngAfterViewChecked() {
        // This seems to be required to activate the dropdowns when used in
        // an event table row. Probably something to do with the stopPropagations.
        $(".dropdown-toggle").dropdown();
    }

    buildState(): any {
        const state: AlertsState = {
            rows: this.rows,
            allRows: this.allRows,
            activeRow: this.activeRow,
            queryString: this.queryString,
            route: this.appService.getRoute(),
            scrollOffset: window.pageYOffset,
        };
        return state;
    }

    restoreState(): boolean {
        const state: AlertsState = this.alertService.popState();
        if (!state) {
            return false;
        }

        console.log("Restoring previous state.");

        if (state.route != this.appService.getRoute()) {
            console.log("Saved state route differs.");
            return false;
        }
        if (state.queryString != this.queryString) {
            console.log("Query strings differ, previous state not being restored.");
            return false;
        }

        this.rows = state.rows;
        this.allRows = state.allRows;
        this.activeRow = state.activeRow;

        // If in inbox, remove any archived events.
        if (this.isInbox()) {
            const archived = this.rows.filter((row: any) => {
                return row.event.event._source.tags.indexOf("archived") > -1;
            });

            console.log(`Found ${archived.length} archived events.`);

            archived.forEach((row: any) => {
                this.removeRow(row);
            });
        } else if (this.appService.getRoute() == "/escalated") {
            const deEscalated = this.rows.filter(row => {
                return row.event.escalatedCount == 0;
            });

            deEscalated.forEach(row => {
                this.removeRow(row);
            });
        }

        setTimeout(() => {
            window.scrollTo(0, state.scrollOffset);
        }, 0);

        return true;
    }

    isInbox() {
        return this.appService.getRoute() == "/inbox";
    }

    older() {
        this.offset += this.windowSize;
        this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
    }

    oldest() {
        while (this.offset + this.windowSize < this.allRows.length) {
            this.offset += this.windowSize;
        }
        this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
    }

    newest() {
        this.offset = 0;
        this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
    }

    newer() {
        if (this.offset > this.windowSize) {
            this.offset -= this.windowSize;
        } else {
            this.offset = 0;
        }
        this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
    }

    showAll() {
        this.rows = this.allRows;
    }

    min(a: number, b: number): number {
        return Math.min(a, b);
    }

    private compare(a: any, b: any): number {
        if (a < b) {
            return -1;
        } else if (a > b) {
            return 1;
        }
        return 0;
    }

    onSort(column: string) {
        console.log("Sorting by: " + column);


        if (column != this.sortBy) {
            this.sortBy = column;
            this.sortOrder = "desc";
        } else {
            if (this.sortOrder == "desc") {
                this.sortOrder = "asc";
            } else {
                this.sortOrder = "desc";
            }
        }

        this.appService.updateParams(this.route, {sortBy: this.sortBy, sortOrder: this.sortOrder});
        this.sort();
    }

    sort() {
        switch (this.sortBy) {
            case "signature":
                this.allRows.sort((a: any, b: any) => {
                    return this.compare(
                        a.event.event._source.alert.signature.toUpperCase(),
                        b.event.event._source.alert.signature.toUpperCase());
                });
                break;
            case "count":
                this.allRows.sort((a: any, b: any) => {
                    return a.event.count - b.event.count;
                });
                break;
            case "source":
                this.allRows.sort((a: any, b: any) => {
                    return this.compare(
                        a.event.event._source.src_ip,
                        b.event.event._source.src_ip);
                });
                break;
            case "dest":
                this.allRows.sort((a: any, b: any) => {
                    return this.compare(
                        a.event.event._source.dest_ip,
                        b.event.event._source.dest_ip);
                });
                break;
            case "timestamp":
                this.allRows.sort((a: any, b: any) => {
                    return this.compare(a.event.maxTs, b.event.maxTs);
                });
                break;
        }

        if (this.sortOrder === "desc") {
            this.allRows.reverse();
        }

        this.rows = this.allRows.slice(this.offset, this.windowSize);
    }

    escalateAndArchiveEvent(row: any) {
        this.archiveAlertGroup(row).then(() => {
            this.escalateAlertGroup(row);
        });
    }

    appEventHandler(event: AppEvent) {
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

    toggleSelectedState(row: any) {
        row.selected = !row.selected;
    }

    escalateSelected() {
        const selected = this.rows.filter((row: any) => {
            return row.selected;
        });
        selected.forEach((row: any) => {

            // Optimistically mark as all escalated.
            row.event.escalatedCount = row.event.count;

            this.elasticSearchService.escalateAlertGroup(row.event);
        });
    }

    archiveSelected() {
        const selected = this.rows.filter((row: any) => {
            return row.selected &&
                row.event.event._source.tags &&
                row.event.event._source.tags.indexOf("archived") < 0;
        });
        selected.forEach((row: any) => {
            this.archiveAlertGroup(row);
        });
    }

    archiveAlertGroup(row: any) {

        if (!row) {
            return;
        }

        // Optimistically mark the event as archived.
        if (!row.event.event._source.tags) {
            row.event.event._source.tags = [];
        }
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
        } else if (this.getActiveRowIndex() > -1) {
            this.archiveAlertGroup(this.getActiveRow());
        }
    }

    removeRow(row: any) {
        const rowIndex = this.rows.indexOf(row);

        if (this.rows === this.allRows) {
            this.allRows = this.allRows.filter((_row: any) => {
                if (_row == row) {
                    return false;
                }
                return true;
            });
            this.rows = this.allRows;
        } else {
            // Remove the list of all alerts.
            this.allRows = this.allRows.filter((_row: any) => {
                if (_row == row) {
                    return false;
                }
                return true;
            });

            // Remove the event from the visible alerts.
            this.rows = this.rows.filter((_row: any) => {
                if (_row == row) {
                    return false;
                }
                return true;
            });

            // Attempt to slide in an event from the next page.
            this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
        }

        // If out of rows, attempt to slide in a portion of the complete result
        // set.
        if (this.rows.length == 0) {
            if (this.offset > 0) {
                this.offset -= this.windowSize;
            }
            this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
            this.activeRow = 0;

            // And scroll to the top.
            window.scrollTo(0, 0);
        } else {
            // Otherwise, do some updating of the active row.
            if (rowIndex < this.activeRow) {
                this.activeRow--;
            }

            if (this.activeRow >= this.rows.length) {
                this.activeRow--;
            }
        }
    }

    focusFilterInput() {
        document.getElementById("filter-input").focus();
    }

    /**
     * Return true if all rows are selected.
     */
    allSelected() {
        return this.rows.every((row: any) => {
            return row.selected;
        });
    }

    getSelectedRows() {
        return this.rows.filter((row: any) => {
            return row.selected;
        });
    }

    getSelectedCount() {
        return this.getSelectedRows().length;
    }

    selectAllRows() {
        this.rows.forEach((row: any) => {
            row.selected = true;
        });
    }

    deselectAllRows() {
        this.rows.forEach((row: any) => {
            row.selected = false;
        });
    }

    submitFilter() {
        const queryParams: any = {};
        if (this.queryString !== "") {
            queryParams.q = this.queryString;
        }
        this.router.navigate([], {
            queryParams,
        });
        document.getElementById("filter-input").blur();
    }

    clearFilter() {
        this.queryString = "";
        this.submitFilter();
    }

    openEvent(event: AlertGroup) {

        // Save the current state of this.
        this.alertService.pushState(this.buildState());

        this.eventService.pushAlertGroup(event);
        this.router.navigate(["/event", event.event._id, {
            referer: this.appService.getRoute()
        }]);

    }

    rowClicked(row: any) {
        this.openEvent(row.event);
    }

    toggleEscalatedState(row: any, event?: any) {

        if (event) {
            event.stopPropagation();
        }

        const alertGroup: AlertGroup = row.event;

        if (alertGroup.escalatedCount < alertGroup.count) {

            // Optimistically mark as all escalated.
            alertGroup.escalatedCount = alertGroup.count;

            this.elasticSearchService.escalateAlertGroup(alertGroup);
        } else if (alertGroup.escalatedCount == alertGroup.count) {

            // Optimistically mark all as de-escalated.
            alertGroup.escalatedCount = 0;

            this.elasticSearchService.removeEscalatedStateFromAlertGroup(alertGroup);
        }
    }

    escalateAlertGroup(row: any) {

        const alertGroup: any = row.event;

        // Optimistically mark as all escalated.
        alertGroup.escalatedCount = alertGroup.count;

        return this.elasticSearchService.escalateAlertGroup(alertGroup);
    }

    refresh() {
        console.log("Refreshing...");
        this.loading = true;

        const queryOptions: any = {
            mustHaveTags: [],
            mustNotHaveTags: [],
            timeRange: "",
            queryString: this.queryString,
        };

        // Add filters depending on view.
        switch (this.appService.getRoute()) {
            case "/inbox":
                // Limit to non-archived events.
                queryOptions.mustNotHaveTags.push("archived");
                break;
            case "/escalated":
                // Limit to escalated events only, no time range applied.
                queryOptions.mustHaveTags.push("escalated");
                break;
            default:
                break;
        }

        // Set a time range on all but escalated.
        switch (this.appService.getRoute()) {
            case "/escalated":
                break;
            default:
                if (this.topNavService.timeRange) {
                    queryOptions.timeRange = `${this.topNavService.getTimeRangeAsSeconds()}s`;
                }
                break;
        }

        return this.elasticSearchService.getAlerts(queryOptions).then((rows: any) => {
            this.allRows = rows;
            this.offset = 0;
            this.rows = this.allRows.slice(this.offset, this.windowSize);
        }, (error: any) => {
            console.log("error handler");

            if (error === false) {
                console.log("Got error 'false', ignoring.");
                return;
            }

            this.rows = [];

            if (typeof error === "object") {
                if (error.error.message) {
                    this.toastr.error(error.error.message);
                    return;
                }
            }

            // Check for a reason.
            try {
                this.toastr.error(error.message);
            } catch (err) {
                this.toastr.error("An error occurred while executing query.");
            }


        }).then(() => {
            this.sort();
            this.activeRow = 0;
            setTimeout(() => {
                this.loading = false;
            }, 0);
            this.appService.resetIdleTime();
        });
    }

    // Event handler to show the dropdown menu for the active row.
    openDropdownMenu() {
        // Toggle.
        const element = $("#row-" + this.activeRow + " .dropdown-toggle");
        element.dropdown("toggle");

        // Focus.
        element.next().find("a:first").focus();
    }

    isArchived(row: any) {
        if (row.event.event._source.tags) {
            if (row.event.event._source.tags.indexOf("archived") > -1) {
                return true;
            }
        }
        return false;
    }

    selectBySignatureId(row: any) {
        const signatureId = row.event.event._source.alert.signature_id;

        this.rows.forEach((row: any) => {
            if (row.event.event._source.alert.signature_id === signatureId) {
                row.selected = true;
            }
        });

        // Close the dropdown. Bootstraps toggle method didn't work quite
        // right here, so remove the "show" class.
        $(".dropdown-menu.show").removeClass("show");
    }

    filterBySignatureId(row: any) {
        const signatureId = row.event.event._source.alert.signature_id;
        this.appService.updateParams(this.route, {
            q: `alert.signature_id:${signatureId}`
        });

        // Close the dropdown. Bootstraps toggle method didn't work quite
        // right here, so remove the "show" class.
        $(".dropdown-menu.show").removeClass("show");
    }

}
