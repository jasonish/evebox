// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { AlertService } from "./alert.service";
import { AfterViewChecked, Component, OnDestroy, OnInit } from "@angular/core";
import { AlertGroup, ElasticSearchService } from "./elasticsearch.service";
import { ActivatedRoute, Router } from "@angular/router";
import { MousetrapService } from "./mousetrap.service";
import { AppEvent, AppEventCode, AppService } from "./app.service";
import { EventService } from "./event.service";
import { ToastrService } from "./toastr.service";
import { TopNavService } from "./topnav.service";
import { loadingAnimation } from "./animations";
import { SETTING_ALERTS_PER_PAGE, SettingsService } from "./settings.service";
import { debounce } from "rxjs/operators";
import { combineLatest, interval } from "rxjs";
import { transformEcsEvent } from "./events/events.component";
import * as moment from "moment";
import { ApiService } from "./api.service";

declare var window: any;
import $ from "jquery";

export interface AlertsState {
  rows: any[];
  allRows: any[];
  activeRow: number;
  route: string;
  queryString: string;
  scrollOffset: number;
}

const DEFAULT_SORT_ORDER = "desc";
const DEFAULT_SORT_BY = "timestamp";

@Component({
  templateUrl: "./alerts.component.html",
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

  constructor(
    private alertService: AlertService,
    private elasticSearchService: ElasticSearchService,
    private router: Router,
    private route: ActivatedRoute,
    private mousetrap: MousetrapService,
    private appService: AppService,
    private eventService: EventService,
    private toastr: ToastrService,
    private topNavService: TopNavService,
    private api: ApiService,
    private settings: SettingsService
  ) {}

  ngOnInit(): any {
    this.windowSize = this.settings.getInt(SETTING_ALERTS_PER_PAGE, 100);

    combineLatest([this.route.queryParams, this.route.params])
      .pipe(debounce(() => interval(100)))
      .subscribe(([queryParams, params]) => {
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
      this.toggleEscalatedState(this.getActiveRow())
    );

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
      this.toggleSelectedState(this.getActiveRow())
    );
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

  ngAfterViewChecked(): void {
    // This seems to be required to activate the dropdowns when used in
    // an event table row. Probably something to do with the stopPropagations.
    // TODO: Bootstrap5
    //$(".dropdown-toggle").dropdown();
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

    if (state.route !== this.appService.getRoute()) {
      console.log("Saved state route differs.");
      return false;
    }
    if (state.queryString !== this.queryString) {
      console.log("Query strings differ, previous state not being restored.");
      return false;
    }

    this.rows = state.rows;
    this.allRows = state.allRows;
    this.activeRow = state.activeRow;

    // If in inbox, remove any archived events.
    if (this.isInbox()) {
      const archived = this.rows.filter((row: any) => {
        return (
          row.event.event._source.tags &&
          row.event.event._source.tags.indexOf("archived") > -1
        );
      });

      console.log(`Found ${archived.length} archived events.`);

      archived.forEach((row: any) => {
        this.removeRow(row);
      });
    } else if (this.appService.getRoute() === "/escalated") {
      const deEscalated = this.rows.filter((row) => {
        return row.event.escalatedCount === 0;
      });

      deEscalated.forEach((row) => {
        this.removeRow(row);
      });
    }

    setTimeout(() => {
      window.scrollTo(0, state.scrollOffset);
    }, 0);

    return true;
  }

  isInbox(): boolean {
    return this.appService.getRoute() === "/inbox";
  }

  older(): void {
    this.offset += this.windowSize;
    this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
  }

  oldest(): void {
    while (this.offset + this.windowSize < this.allRows.length) {
      this.offset += this.windowSize;
    }
    this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
  }

  newest(): void {
    this.offset = 0;
    this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
  }

  newer(): void {
    if (this.offset > this.windowSize) {
      this.offset -= this.windowSize;
    } else {
      this.offset = 0;
    }
    this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
  }

  showAll(): void {
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

  onSort(column: string): void {
    console.log("Sorting by: " + column);

    if (column !== this.sortBy) {
      this.sortBy = column;
      this.sortOrder = "desc";
    } else {
      if (this.sortOrder === "desc") {
        this.sortOrder = "asc";
      } else {
        this.sortOrder = "desc";
      }
    }

    this.appService.updateParams(this.route, {
      sortBy: this.sortBy,
      sortOrder: this.sortOrder,
    });
    this.sort();
  }

  sort(): void {
    switch (this.sortBy) {
      case "signature":
        this.allRows.sort((a: any, b: any) => {
          return this.compare(
            a.event.event._source.alert.signature.toUpperCase(),
            b.event.event._source.alert.signature.toUpperCase()
          );
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
            b.event.event._source.src_ip
          );
        });
        break;
      case "dest":
        this.allRows.sort((a: any, b: any) => {
          return this.compare(
            a.event.event._source.dest_ip,
            b.event.event._source.dest_ip
          );
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

  escalateAndArchiveEvent(row: any): void {
    this.archiveAlertGroup(row).then(() => {
      this.escalateAlertGroup(row);
    });
  }

  appEventHandler(event: AppEvent): void {
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

        if (this.rows.length === 0 && event.data < 5) {
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

        this.refresh(true);
        break;
    }
  }

  openActiveEvent(): void {
    this.openEvent(this.getActiveRow().event);
  }

  archiveActiveEvent(): void {
    if (this.getActiveRowIndex() >= 0) {
      this.archiveAlertGroup(this.getActiveRow());
    }
  }

  getActiveRow(): any {
    return this.rows[this.getActiveRowIndex()];
  }

  getActiveRowIndex(): number {
    return this.activeRow;
  }

  toggleSelectedState(row: any): void {
    row.selected = !row.selected;
  }

  escalateSelected(): void {
    const selected = this.rows.filter((row: any) => {
      return row.selected;
    });
    selected.forEach((row: any) => {
      // Optimistically mark as all escalated.
      row.event.escalatedCount = row.event.count;

      this.elasticSearchService.escalateAlertGroup(row.event);
    });
  }

  archiveSelected(): void {
    const selected = this.rows.filter((row: any) => {
      return (
        row.selected &&
        row.event.event._source.tags &&
        row.event.event._source.tags.indexOf("archived") < 0
      );
    });
    selected.forEach((row: any) => {
      this.archiveAlertGroup(row);
    });
  }

  archiveAlertGroup(row: any): Promise<any> {
    if (!row) {
      return;
    }

    // Optimistically mark the event as archived.
    if (!row.event.event._source.tags) {
      row.event.event._source.tags = [];
    }
    row.event.event._source.tags.push("archived");

    // If in inbox, also remove it from view.
    if (this.appService.getRoute() === "/inbox") {
      this.removeRow(row);
    }

    return this.elasticSearchService.archiveAlertGroup(row.event);
  }

  archiveEvents(): void {
    // If rows are selected, archive the selected rows, otherwise archive
    // the current active event.
    if (this.getSelectedCount() > 0) {
      this.archiveSelected();
    } else if (this.getActiveRowIndex() > -1) {
      this.archiveAlertGroup(this.getActiveRow());
    }
  }

  removeRow(row: any): void {
    const rowIndex = this.rows.indexOf(row);

    if (this.rows === this.allRows) {
      this.allRows = this.allRows.filter((row0: any) => {
        if (row0 === row) {
          return false;
        }
        return true;
      });
      this.rows = this.allRows;
    } else {
      // Remove the list of all alerts.
      this.allRows = this.allRows.filter((row0: any) => {
        if (row0 === row) {
          return false;
        }
        return true;
      });

      // Remove the event from the visible alerts.
      this.rows = this.rows.filter((row0: any) => {
        if (row0 === row) {
          return false;
        }
        return true;
      });

      // Attempt to slide in an event from the next page.
      this.rows = this.allRows.slice(
        this.offset,
        this.offset + this.windowSize
      );
    }

    // If out of rows, attempt to slide in a portion of the complete result
    // set.
    if (this.rows.length === 0) {
      if (this.offset > 0) {
        this.offset -= this.windowSize;
      }
      this.rows = this.allRows.slice(
        this.offset,
        this.offset + this.windowSize
      );
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

  focusFilterInput(): void {
    document.getElementById("filter-input").focus();
  }

  /**
   * Return true if all rows are selected.
   */
  allSelected(): boolean {
    return this.rows.every((row: any) => {
      return row.selected;
    });
  }

  getSelectedRows(): any[] {
    return this.rows.filter((row: any) => {
      return row.selected;
    });
  }

  getSelectedCount(): number {
    return this.getSelectedRows().length;
  }

  selectAllRows(): void {
    this.rows.forEach((row: any) => {
      row.selected = true;
    });
  }

  deselectAllRows(): void {
    this.rows.forEach((row: any) => {
      row.selected = false;
    });
  }

  submitFilter(): void {
    const queryParams: any = {};
    if (this.queryString !== "") {
      queryParams.q = this.queryString;
    }
    this.router.navigate([], {
      queryParams,
    });
    (<HTMLElement>document.activeElement).blur();
  }

  clearFilter(): void {
    this.queryString = "";
    this.submitFilter();
  }

  openEvent(event: AlertGroup): void {
    // Save the current state of this.
    this.alertService.pushState(this.buildState());

    this.eventService.pushAlertGroup(event);
    this.router.navigate([
      "/event",
      event.event._id,
      {
        referer: this.appService.getRoute(),
      },
    ]);
  }

  rowClicked(row: any): void {
    this.openEvent(row.event);
  }

  toggleEscalatedState(row: any, event?: any): void {
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

  escalateAlertGroup(row: any): Promise<void> {
    const alertGroup: any = row.event;

    // Optimistically mark as all escalated.
    alertGroup.escalatedCount = alertGroup.count;

    return this.elasticSearchService.escalateAlertGroup(alertGroup);
  }

  refresh(silent = false): void {
    this.loading = true;
    this.silentRefresh = silent;

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

    this.api
      .alertQuery(queryOptions)
      .toPromise()
      .then(
        (response: any) => {
          const rows = response.alerts.map((alert: AlertGroup) => {
            if (response.ecs) {
              transformEcsEvent(alert.event);
            }
            return {
              event: alert,
              selected: false,
              date: moment(alert.maxTs).toDate(),
              ecs: response.ecs,
            };
          });

          this.allRows = rows;
          this.rows = this.allRows.slice(this.offset, this.windowSize);
          this.offset = 0;
          this.activeRow = 0;
          this.sort();
          setTimeout(() => {
            this.loading = false;
          }, 0);
          this.appService.resetIdleTime();
          this.silentRefresh = false;
        },
        (error: any) => {
          this.silentRefresh = false;
          this.loading = false;
          if (error === false) {
            console.log("Got error 'false', ignoring.");
            return;
          }

          this.rows = [];

          try {
            if (error.error.error) {
              console.log(error.error.error);
              this.toastr.error(error.error.error);
              return;
            }
          } catch (e) {}

          // Check for a reason.
          try {
            this.toastr.error(error.message);
          } catch (err) {
            this.toastr.error("An error occurred while executing query.");
          }
        }
      );
  }

  // Event handler to show the dropdown menu for the active row.
  openDropdownMenu(): void {
    // Toggle.
    const element = $("#row-" + this.activeRow + " .dropdown-toggle");
    element.dropdown("toggle");

    // Focus.
    element.next().find("a:first").focus();
  }

  isArchived(row: any): boolean {
    if (row.event.event._source.tags) {
      if (row.event.event._source.tags.indexOf("evebox.archived") > -1) {
        return true;
      }
      if (row.event.event._source.tags.indexOf("archived") > -1) {
        return true;
      }
    }
    return false;
  }

  selectBySignatureId(row: any): void {
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

  filterBySignatureId(row: any): void {
    const signatureId = row.event.event._source.alert.signature_id;
    this.appService.updateParams(this.route, {
      q: `alert.signature_id:${signatureId}`,
    });

    // Close the dropdown. Bootstraps toggle method didn't work quite
    // right here, so remove the "show" class.
    $(".dropdown-menu.show").removeClass("show");
  }
}
