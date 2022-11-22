// Copyright (C) 2016-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Component, OnDestroy, OnInit } from "@angular/core";
import { ActivatedRoute, Params } from "@angular/router";
import { ReportsService } from "../reports.service";
import { AppEvent, AppEventCode, AppService } from "../../app.service";
import { ToastrService } from "../../toastr.service";
import { TopNavService } from "../../topnav.service";
import { ElasticSearchService } from "../../elasticsearch.service";
import { EveboxSubscriptionTracker } from "../../subscription-tracker";
import { loadingAnimation } from "../../animations";
import { ApiService, ReportAggOptions } from "../../api.service";

@Component({
  templateUrl: "./netflow-report.component.html",
  animations: [loadingAnimation],
})
export class NetflowReportComponent implements OnInit, OnDestroy {
  topBySourceIp: any[];
  topByDestIp: any[];

  topBySourcePort: any[];
  topByDestPort: any[];

  loading = 0;

  queryString = "";

  subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

  constructor(
    private reportsService: ReportsService,
    private elasticsearch: ElasticSearchService,
    private appService: AppService,
    private route: ActivatedRoute,
    private toastr: ToastrService,
    private api: ApiService,
    private topNavService: TopNavService
  ) {}

  ngOnInit() {
    this.route.queryParams.subscribe((params: Params) => {
      this.queryString = params["q"] || "";
      this.refresh();
    });

    this.subTracker.subscribe(this.appService, (event: AppEvent) => {
      if (event.event === AppEventCode.TIME_RANGE_CHANGED) {
        this.refresh();
      }
    });
  }

  ngOnDestroy() {
    this.subTracker.unsubscribe();
  }

  refresh() {
    this.load();
  }

  private wrapPromise(fn: any) {
    this.loading++;
    fn().then(() => {
      this.loading--;
    });
  }

  load() {
    let range = this.topNavService.getTimeRangeAsSeconds();

    const aggOptions: ReportAggOptions = {
      timeRange: range,
      eventType: "netflow",
      size: 10,
      queryString: this.queryString,
    };

    this.wrapPromise(() => {
      return this.api.reportAgg("src_ip", aggOptions).then((response) => {
        this.topBySourceIp = response.data;
      });
    });

    this.wrapPromise(() => {
      return this.api.reportAgg("dest_ip", aggOptions).then((response: any) => {
        this.topByDestIp = response.data;
      });
    });

    this.wrapPromise(() => {
      return this.api.reportAgg("src_port", aggOptions).then((response) => {
        this.topBySourcePort = response.data;
      });
    });

    this.wrapPromise(() => {
      return this.api
        .reportAgg("dest_port", aggOptions)
        .then((response: any) => {
          this.topByDestPort = response.data;
        });
    });
  }
}
