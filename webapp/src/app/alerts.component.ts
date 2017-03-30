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

import {AlertService} from './alert.service';
import {Component, OnInit, OnDestroy} from '@angular/core';
import {ElasticSearchService, AlertGroup} from './elasticsearch.service';
import {Router, ActivatedRoute} from '@angular/router';
import {MousetrapService} from './mousetrap.service';
import {AppService, AppEvent, AppEventCode} from './app.service';
import {EventService} from './event.service';
import {ToastrService} from './toastr.service';
import {TopNavService} from './topnav.service';
import {EveboxSubscriptionService} from './subscription.service';
import {loadingAnimation} from './animations';

declare var window: any;

export interface AlertsState {
    rows: any[];
    allRows: any[];
    activeRow: number;
    route: string;
    queryString: string;
    scrollOffset: number;
}

@Component({
    templateUrl: './alerts.component.html',
    animations: [
        loadingAnimation,
    ]
})
export class AlertsComponent implements OnInit, OnDestroy {

    windowSize = 100;
    offset = 0;

    rows: any[] = [];
    allRows: any[] = [];

    activeRow = 0;
    queryString = '';
    loading = false;
    dispatcherSubscription: any;

    silentRefresh = false;

    constructor(private alertService: AlertService,
                private elasticSearchService: ElasticSearchService,
                private router: Router,
                private route: ActivatedRoute,
                private mousetrap: MousetrapService,
                private appService: AppService,
                private eventService: EventService,
                private toastr: ToastrService,
                private ss: EveboxSubscriptionService,
                private topNavService: TopNavService) {
    }

    ngOnInit(): any {

        this.ss.subscribe(this, this.route.params, (params: any) => {
            this.queryString = params.q || '';
            if (!this.restoreState()) {
                this.refresh();
            }
        });

        this.mousetrap.bind(this, '/', () => this.focusFilterInput());
        this.mousetrap.bind(this, '* a', () => this.selectAllRows());
        this.mousetrap.bind(this, '* n', () => this.deselectAllRows());
        this.mousetrap.bind(this, 'r', () => this.refresh());
        this.mousetrap.bind(this, 'o', () => this.openActiveEvent());
        this.mousetrap.bind(this, 'f8', () => this.archiveActiveEvent());
        this.mousetrap.bind(this, 's', () =>
            this.toggleEscalatedState(this.getActiveRow()));

        // Escalate then archive event.
        this.mousetrap.bind(this, 'f9', () => {
            this.escalateAndArchiveEvent(this.getActiveRow());
        });

        this.mousetrap.bind(this, 'x', () =>
            this.toggleSelectedState(this.getActiveRow()));
        this.mousetrap.bind(this, 'e', () => this.archiveEvents());

        this.mousetrap.bind(this, '>', () => {
            this.older();
        });

        this.mousetrap.bind(this, '<', () => {
            this.newer();
        });

        // CTRL >
        this.mousetrap.bind(this, 'ctrl+shift+.', () => {
            this.oldest();
        });

        // CTRL <
        this.mousetrap.bind(this, 'ctrl+shift+,', () => {
            this.newest();
        });

        this.dispatcherSubscription = this.appService.subscribe((event: any) => {
            this.appEventHandler(event);
        });
    }

    ngOnDestroy(): any {
        this.mousetrap.unbind(this);
        this.ss.unsubscribe(this);
        this.dispatcherSubscription.unsubscribe();
    }

    buildState(): any {
        let state: AlertsState = {
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

        let state: AlertsState = this.alertService.popState();
        if (!state) {
            return false;
        }

        console.log('Restoring previous state.');

        if (state.route != this.appService.getRoute()) {
            console.log('Saved state route differs.');
            return false;
        }
        if (state.queryString != this.queryString) {
            console.log('Query strings differ, previous state not being restored.');
            return false;
        }

        this.rows = state.rows;
        this.allRows = state.allRows;
        this.activeRow = state.activeRow;

        // If in inbox, remove any archived events.
        if (this.isInbox()) {
            let archived = this.rows.filter((row: any) => {
                return row.event.event._source.tags.indexOf('archived') > -1;
            });

            archived.forEach((row: any) => {
                this.removeRow(row);
            });
        }
        else if (this.appService.getRoute() == '/escalated') {
            let deEscalated = this.rows.filter(row => {
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
        return this.appService.getRoute() == '/inbox';
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
        }
        else {
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
                    console.log('Elastic Search jobs active, not refreshing.');
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
        let selected = this.rows.filter((row: any) => {
            return row.selected;
        });
        selected.forEach((row: any) => {

            // Optimistically mark as all escalated.
            row.event.escalatedCount = row.event.count;

            this.elasticSearchService.escalateAlertGroup(row.event);
        });
    }

    archiveSelected() {
        let selected = this.rows.filter((row: any) => {
            return row.selected &&
                row.event.event._source.tags.indexOf('archived') < 0;
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
        row.event.event._source.tags.push('archived');

        // If in inbox, also remove it from view.
        if (this.appService.getRoute() == '/inbox') {
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

    removeRow(row: any) {

        // Remove the event from the visible events.
        this.rows = this.rows.filter((_row: any) => {
            if (_row == row) {
                return false;
            }
            return true;
        });

        // Remove from the all event store as well.
        this.allRows = this.allRows.filter((_row: any) => {
            if (_row == row) {
                return false;
            }
            return true;
        });

        // Attempt to slide in an event from the next page.
        this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);

        if (this.activeRow >= this.rows.length) {
            this.activeRow--;
        }

        if (this.rows.length == 0) {
            if (this.offset < this.allRows.length) {
            }
            else if (this.offset > 0) {
                this.offset -= this.windowSize;
            }
            this.rows = this.allRows.slice(this.offset, this.offset + this.windowSize);
            this.activeRow = 0;

            // And scroll to the top.
            window.scrollTo(0, 0);
        }
    }

    focusFilterInput() {
        document.getElementById('filter-input').focus();
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
        //this.appService.updateQueryParameters({q: this.queryString});
        this.appService.updateParams(this.route, {q: this.queryString});
        document.getElementById('filter-input').blur();
        this.refresh();
    }

    openEvent(event: AlertGroup) {

        // Save the current state of this.
        this.alertService.pushState(this.buildState());

        this.eventService.pushAlertGroup(event);
        this.router.navigate(['/event', event.event._id, {
            referer: this.appService.getRoute()
        }]);
    }

    rowClicked(row: any) {
        this.openEvent(row.event);
    }

    clearFilter() {
        this.queryString = '';
        this.submitFilter();
    }

    toggleEscalatedState(row: any, event?: any) {

        if (event) {
            event.stopPropagation();
        }

        let alertGroup: AlertGroup = row.event;

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

    escalateAlertGroup(row: any) {

        let alertGroup: any = row.event;

        // Optimistically mark as all escalated.
        alertGroup.escalatedCount = alertGroup.count;

        return this.elasticSearchService.escalateAlertGroup(alertGroup);
    }

    refresh() {

        this.loading = true;

        let queryOptions: any = {
            mustHaveTags: [],
            mustNotHaveTags: [],
            timeRange: '',
            queryString: this.queryString,
        };

        // Add filters depending on view.
        switch (this.appService.getRoute()) {
            case '/inbox':
                // Limit to non-archived events.
                queryOptions.mustNotHaveTags.push('archived');
                break;
            case '/escalated':
                // Limit to escalated events only, no time range applied.
                queryOptions.mustHaveTags.push('escalated');
                break;
            default:
                break;
        }

        let range = 0;

        // Set a time range on all but escalated.
        switch (this.appService.getRoute()) {
            case '/escalated':
                break;
            default:
                //queryOptions.timeRange = this.topNavService.timeRange;
                if (this.topNavService.timeRange) {
                    queryOptions.timeRange = `${this.topNavService.getTimeRangeAsSeconds()}s`;
                }
                break;
        }

        return this.elasticSearchService.newGetAlerts(queryOptions).then((rows: any) => {
            this.allRows = rows;
            this.offset = 0;
            this.rows = this.allRows.slice(this.offset, this.windowSize);
        }, (error: any) => {

            this.rows = [];

            if (typeof error === 'object') {
                if (error.message) {
                    this.toastr.error(error.message);
                    return;
                }
            }

            console.log('Error fetching alerts:');
            console.log(error);

            // Check for a reason.
            try {
                this.toastr.error(error.error.root_cause[0].reason);
            }
            catch (err) {
                this.toastr.error('An error occurred while executing query.');
            }


        }).then(() => {
            this.activeRow = 0;
            this.loading = false;
            this.appService.resetIdleTime();
        });
    }

}
