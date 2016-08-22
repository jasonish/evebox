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

import {Component, OnInit, OnDestroy} from "@angular/core";
import {ActivatedRoute} from "@angular/router";
import {ElasticSearchService, ResultSet} from "./elasticsearch.service";
import {
    EveboxEventTableComponent,
    EveboxEventTableConfig
} from "./event-table.component";
import {MousetrapService} from "./mousetrap.service";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";
import {AppService} from "./app.service";
import {ToastrService} from "./toastr.service";
import {EveboxSubscriptionService} from "./subscription.service";

@Component({
    template: `<div [ngClass]="{'evebox-opacity-50': loading}">
  <div class="row">
    <div class="col-md-12">
      <div class="form-group">
        <form name="filterInputForm" (submit)="submitFilter()">
          <div class="input-group">
            <input id="filter-input" type="text" class="form-control"
                   placeholder="Filter..." [(ngModel)]="queryString"
                   name="queryString"/>
            <div class="input-group-btn">
              <button type="submit" class="btn btn-default">Search</button>
              <button type="button" class="btn btn-default"
                      (click)="clearFilter()">Clear
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  </div>

  <div class="row">
    <div class="col-md-12">
      <button type="button" class="btn btn-default" (click)="refresh()">Refresh
      </button>

      <div *ngIf="hasEvents()" class="pull-right">
        <button type="button" class="btn btn-default" (click)="gotoNewest()">
          Newest
        </button>
        <button type="button" class="btn btn-default" (click)="gotoOlder()">
          Older
        </button>
      </div>

    </div>
  </div>

  <loading-spinner [loading]="loading"></loading-spinner>

  <div *ngIf="!loading && !hasEvents()" style="text-align: center;">
    <hr/>
    No events found.
    <hr/>
  </div>

  <br/>

  <eveboxEventTable
      [config]="eveboxEventTableConfig"></eveboxEventTable>
</div>`,
    directives: [
        EveboxEventTableComponent,
        EveboxLoadingSpinnerComponent
    ]
})
export class EventsComponent implements OnInit, OnDestroy {

    private resultSet:ResultSet;

    private loading:boolean = false;

    private queryString:string = "";

    private timeStart:string;
    private timeEnd:string;

    private eveboxEventTableConfig:EveboxEventTableConfig = {
        showCount: false,
        rows: []
    };

    constructor(private route:ActivatedRoute,
                private elasticsearch:ElasticSearchService,
                private mousetrap:MousetrapService,
                private appService:AppService,
                private toastr:ToastrService,
                private ss:EveboxSubscriptionService) {
    }

    ngOnInit():any {

        this.ss.subscribe(this, this.route.params, (params:any) => {
            this.queryString = params.q || "";
            this.timeStart = params.timeStart;
            this.timeEnd = params.timeEnd;
            this.refresh();
        });

        this.appService.disableTimeRange();

        this.mousetrap.bind(this, "/", () => this.focusFilterInput());
        this.mousetrap.bind(this, "r", () => this.refresh());
    }

    ngOnDestroy() {
        this.mousetrap.unbind(this);
        this.ss.unsubscribe(this);
    }

    focusFilterInput() {
        document.getElementById("filter-input").focus();
    }

    submitFilter() {
        document.getElementById("filter-input").blur();
        this.appService.updateParams(this.route, {
            q: this.queryString
        });
        //this.router.navigate([this.appService.getRoute(), {q: this.queryString}]);
    }

    clearFilter() {
        this.queryString = "";
        this.submitFilter();
    }

    gotoNewest() {
        this.appService.updateParams(this.route, {
            timeStart: undefined,
            timeEnd: undefined,
        });
    }

    gotoOlder() {
        this.appService.updateParams(this.route, {
            timeEnd: this.resultSet.oldestTimestamp,
            timeStart: undefined,
        });
    }

    hasEvents() {
        return this.resultSet && this.resultSet.events.length > 0;
    }

    refresh() {

        // Prevent double loading.
        if (this.loading) {
            return;
        }

        this.loading = true;

        this.elasticsearch.findEvents({
            queryString: this.queryString,
            timeEnd: this.timeEnd,
            timeStart: this.timeStart,
        }).then((resultSet:ResultSet) => {
            this.resultSet = resultSet;
            this.eveboxEventTableConfig.rows = resultSet.events.map((event:any) => {
                return event;
            });
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

            this.resultSet = undefined;
            this.eveboxEventTableConfig.rows = [];

        }).then(() => {
            this.loading = false;
        });

    }

}