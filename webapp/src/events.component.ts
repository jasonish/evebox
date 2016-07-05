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
import {Router} from "@angular/router";
import {ElasticSearchService, ResultSet} from "./elasticsearch.service";
import {
    EveboxEventTableComponent,
    EveboxEventTableConfig
} from "./event-table.component";
import {MousetrapService} from "./mousetrap.service";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";
import {AppService} from "./app.service";

@Component({
    template: `<div [ngClass]="{'evebox-opacity-50': loading}">
  <div class="row">
    <div class="col-md-12">
      <div class="form-group">
        <form (submit)="submitFilter()">
          <div class="input-group">
            <input id="filter-input" type="text" class="form-control"
                   placeholder="Filter..." [(ngModel)]="queryString"/>
            <div class="input-group-btn">
              <button type="submit" class="btn btn-default">Search</button>
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

      <div class="pull-right">
        <button type="button" class="btn btn-default" (click)="gotoNewest()">Newest</button>
        <button type="button" class="btn btn-default" (click)="gotoNewer()">Newer</button>
        <button type="button" class="btn btn-default" (click)="gotoOlder()">Older</button>
      </div>

    </div>
  </div>

  <br/>

  <loading-spinner [loading]="loading"></loading-spinner>

  <div class="row">
    <div class="col-md-12">
      <eveboxEventTable
          [config]="eveboxEventTableConfig"></eveboxEventTable>
    </div>
  </div>
</div>
`,
    directives: [
        EveboxEventTableComponent,
        EveboxLoadingSpinnerComponent
    ]
})
export class EventsComponent implements OnInit, OnDestroy {

    private resultSet:ResultSet;

    private loading:boolean = false;

    private queryString:string = "";

    private eveboxEventTableConfig:EveboxEventTableConfig = {
        showCount: false,
        rows: []
    };

    private routerSub:any;

    constructor(private router:Router,
                private elasticsearch:ElasticSearchService,
                private mousetrap:MousetrapService,
                private appService:AppService) {
    }

    ngOnInit():any {

        this.routerSub = this.router.routerState.queryParams.subscribe(
            (params:any) => {
                if (params.q) {
                    this.queryString = params.q;
                }
                else {
                    this.queryString = "";
                }
                this.refresh();
            });

        this.appService.disableTimeRange();

        this.mousetrap.bind(this, "/", () => this.focusFilterInput());
    }

    ngOnDestroy() {
        this.mousetrap.unbind(this);
        this.routerSub.unsubscribe();
    }

    focusFilterInput() {
        document.getElementById("filter-input").focus();
    }

    submitFilter() {
        this.appService.updateQueryParameters({q: this.queryString});
        this.refresh();
    }

    gotoNewest() {
        this.appService.updateQueryParameters({
            timeStart: undefined,
            timeEnd: undefined
        });
        this.refresh();
    }

    gotoNewer() {
        this.appService.updateQueryParameters({
            timeStart: this.resultSet.newestTimestamp,
            timeEnd: undefined
        });
        this.refresh();
    }

    gotoOlder() {
        this.appService.updateQueryParameters({
            timeEnd: this.resultSet.oldestTimestamp,
            timeStart: undefined
        });
        this.refresh();
    }

    refresh() {

        // May be triggered from the filter input, blur the focus.
        document.getElementById("filter-input").blur();

        this.loading = true;
        this.elasticsearch.findEvents({
            queryString: this.queryString,
            timeEnd: this.router.routerState.snapshot.queryParams["timeEnd"],
            timeStart: this.router.routerState.snapshot.queryParams["timeStart"]
        }).then((resultSet:ResultSet) => {
            this.resultSet = resultSet;
            this.eveboxEventTableConfig.rows = resultSet.events.map((event:any) => {
                return event;
            });
            this.loading = false;
        })
    }

}