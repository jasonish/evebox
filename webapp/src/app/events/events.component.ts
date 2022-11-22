// Copyright (C) 2014-2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

import { Component, OnDestroy, OnInit } from "@angular/core";
import { ActivatedRoute, Router } from "@angular/router";
import { ElasticSearchService } from "../elasticsearch.service";
import { MousetrapService } from "../mousetrap.service";
import { AppService } from "../app.service";
import { ToastrService } from "../toastr.service";
import { EveboxSubscriptionService } from "../subscription.service";
import { loadingAnimation } from "../animations";
import { ApiService } from "../api.service";
import { debounce, finalize } from "rxjs/operators";
import { EVENT_TYPES } from "../shared/eventtypes";
import { combineLatest, interval } from "rxjs";

@Component({
  templateUrl: "./events.component.html",
  animations: [loadingAnimation],
})
export class EventsComponent implements OnInit, OnDestroy {
  model: any = {
    newestTimestamp: "",
    oldestTimestamp: "",
    events: [],
  };

  loading = false;

  queryString = "";

  eventTypes = EVENT_TYPES;

  // Error to be display if set.
  error: string = null;

  eventTypeFilter: any = this.eventTypes[0];

  timeStart: string;
  timeEnd: string;
  private order: string;

  constructor(
    private route: ActivatedRoute,
    private router: Router,
    private elasticsearch: ElasticSearchService,
    private mousetrap: MousetrapService,
    private appService: AppService,
    private toastr: ToastrService,
    private api: ApiService
  ) {}

  ngOnInit(): any {
    combineLatest([this.route.queryParams, this.route.params])
      .pipe(debounce(() => interval(100)))
      .subscribe(([queryParams, params]) => {
        let qp: any = this.route.snapshot.queryParams;

        this.timeStart = params.timeStart || qp.timeStart;
        this.timeEnd = params.timeEnd || qp.timeEnd;

        if (params.eventType) {
          this.setEventTypeFilterByEventType(params.eventType);
        }

        this.order = params.order;

        this.queryString = queryParams.q;
        this.refresh();
      });

    // Use setTimeout to prevent ExpressionChangedAfterItHasBeenCheckedError.
    setTimeout(() => {
      this.appService.disableTimeRange();
    }, 0);

    this.mousetrap.bind(this, "/", () => this.focusFilterInput());
    this.mousetrap.bind(this, "r", () => this.refresh());
  }

  setEventTypeFilterByEventType(eventType: string) {
    for (let et of this.eventTypes) {
      if (et.eventType == eventType) {
        this.eventTypeFilter = et;
        break;
      }
    }
  }

  setEventTypeFilter(type: any) {
    this.eventTypeFilter = type;
    this.appService.updateParams(this.route, {
      eventType: this.eventTypeFilter.eventType,
    });
  }

  ngOnDestroy() {
    this.mousetrap.unbind(this);
  }

  focusFilterInput() {
    document.getElementById("filter-input").focus();
  }

  submitFilter() {
    document.getElementById("filter-input").blur();
    this.router.navigate([], {
      queryParams: {
        q: this.queryString,
      },
    });
  }

  clearFilter() {
    this.queryString = "";
    this.submitFilter();
  }

  gotoNewest() {
    this.appService.updateParams(this.route, {
      timeStart: undefined,
      timeEnd: undefined,
      order: "desc",
    });
  }

  gotoNewer() {
    this.appService.updateParams(this.route, {
      timeEnd: undefined,
      timeStart: this.model.newestTimestamp,
      order: "asc",
    });
  }

  gotoOlder() {
    console.log(`gotoOlder: timeEnd=${this.model.oldestTimestamp}`);
    this.appService.updateParams(this.route, {
      timeEnd: this.model.oldestTimestamp,
      timeStart: undefined,
      order: "desc",
    });
  }

  gotoOldest() {
    this.appService.updateParams(this.route, {
      timeEnd: undefined,
      timeStart: undefined,
      order: "asc",
    });
  }

  hasEvents(): boolean {
    try {
      return this.model.events.length > 0;
    } catch (err) {
      return false;
    }
  }

  refresh() {
    this.clearError();
    this.model.events = [];
    this.loading = true;

    this.api
      .eventQuery({
        queryString: this.queryString,
        maxTs: this.timeEnd,
        minTs: this.timeStart,
        eventType: this.eventTypeFilter.eventType,
        sortOrder: this.order,
      })
      .pipe(
        finalize(() => {
          this.loading = false;
        })
      )
      .subscribe(
        (response) => {
          let events = response.data;

          console.log("Got reponse...");

          // If the sortOrder is "asc", reverse to put back into descending sortOrder.
          if (this.order === "asc") {
            events = events.reverse();
          }

          if (response.ecs) {
            console.log("Transforming ECS events...");
            events.forEach((event) => {
              transformEcsEvent(event);
            });
            console.log("Done transforming ECS events...");
          }

          console.log(events[0]);

          if (events.length > 0) {
            this.model.newestTimestamp = events[0]._source["@timestamp"];
            this.model.oldestTimestamp =
              events[events.length - 1]._source["@timestamp"];

            console.log(`Newest event: ${this.model.newestTimestamp}`);
            console.log(`Oldest event: ${this.model.oldestTimestamp}`);
          }
          this.model.events = events;
        },
        (error) => {
          this.setError(error);
        }
      );
  }

  private setError(error: string) {
    this.error = error;
  }

  private clearError() {
    this.error = null;
  }
}

export function transformEcsEvent(event: any): void {
  if (event._transformed) {
    return;
  }
  const original = JSON.parse(event._source.event.original);
  event._ecs = event._source;
  event._source = original;
  event._source.tags = event._ecs.tags;
  event._source["@timestamp"] = event._ecs["@timestamp"];
  event._source.evebox = event._ecs.evebox;
  event._transformed = true;
}
