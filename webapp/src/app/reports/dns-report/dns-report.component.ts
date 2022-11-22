// Copyright (C) 2016-2021 Jason Ish
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
import { ReportsService } from "../reports.service";
import { AppEventCode, AppService } from "../../app.service";
import { EveboxFormatIpAddressPipe } from "../../pipes/format-ipaddress.pipe";
import { EveboxSubscriptionTracker } from "../../subscription-tracker";
import { ActivatedRoute, Params } from "@angular/router";
import { ApiService, ReportAggOptions } from "../../api.service";
import { TopNavService } from "../../topnav.service";

import * as moment from "moment";
import { getCanvasElementById, getColourPalette } from "../../shared/chartjs";
import { Chart, ChartConfiguration } from "chart.js";

@Component({
  templateUrl: "./dns-report.component.html",
})
export class DNSReportComponent implements OnInit, OnDestroy {
  topRrnames: any[];
  topRdata: any[];
  topRrtypes: any[];
  topRcodes: any[];
  topServers: any[];
  topClients: any[];

  loading = 0;

  queryString = "";

  subTracker: EveboxSubscriptionTracker = new EveboxSubscriptionTracker();

  private charts = {
    eventsOverTime: null,
  };

  constructor(
    private route: ActivatedRoute,
    private appService: AppService,
    private api: ApiService,
    private topNavService: TopNavService,
    private reportsService: ReportsService,
    private formatIpAddressPipe: EveboxFormatIpAddressPipe
  ) {}

  ngOnInit(): void {
    this.subTracker.subscribe(this.route.queryParams, (params: Params) => {
      this.queryString = params.q || "";
      this.refresh();
    });

    this.subTracker.subscribe(this.appService, (event: any) => {
      if (event.event === AppEventCode.TIME_RANGE_CHANGED) {
        this.refresh();
      }
    });
  }

  ngOnDestroy(): void {
    this.subTracker.unsubscribe();
    if (this.charts.eventsOverTime != null) {
      this.charts.eventsOverTime.destroy();
    }
  }

  mapAddressAggregation(items: any[]): { count: any; key: any }[] {
    return items.map((item: any) => {
      let key = item.key;

      // If key looks like an IP address, format it.
      if (key.match(/\d*\.\d*\.\d*\.\d*/)) {
        key = this.formatIpAddressPipe.transform(key);
      }

      return {
        key: key,
        count: item.doc_count,
      };
    });
  }

  mapAggregation(items: any[]): { count: any; key: any }[] {
    return items.map((item: any) => {
      return {
        key: item.key,
        count: item.doc_count,
      };
    });
  }

  load(fn: any): void {
    this.loading++;
    fn()
      .then(() => {})
      .catch((err) => {})
      .then(() => {
        this.loading--;
      });
  }

  refresh(): void {
    const size = 10;
    const range = this.topNavService.getTimeRangeAsSeconds();

    const aggOptions: ReportAggOptions = {
      eventType: "dns",
      dnsType: "answer",
      timeRange: range,
      queryString: this.queryString,
      size: size,
    };

    // Top response codes.
    this.load(() => {
      return this.api
        .reportAgg(
          "dns.rcode",
          Object.assign(
            {
              dnsType: "answer",
            },
            aggOptions
          )
        )
        .then((response: any) => {
          this.topRcodes = response.data;
        });
    });

    // Switch to queries.
    aggOptions.dnsType = "query";

    // Top request rrnames.
    this.load(() => {
      return this.api
        .reportAgg("dns.rrname", aggOptions)
        .then((response: any) => {
          this.topRrnames = response.data;
        });
    });

    // Top request rrtypes.
    this.load(() => {
      return this.api
        .reportAgg("dns.rrtype", aggOptions)
        .then((response: any) => {
          this.topRrtypes = response.data;
        });
    });

    // Top DNS clients.
    this.load(() => {
      return this.api.reportAgg("src_ip", aggOptions).then((response: any) => {
        this.topClients = response.data;
      });
    });

    // Top DNS servers.
    this.load(() => {
      return this.api.reportAgg("dest_ip", aggOptions).then((response: any) => {
        this.topServers = response.data;
      });
    });

    // Queries over time histogram.
    this.load(() => {
      return this.api
        .reportHistogram({
          timeRange: range,
          interval: this.reportsService.histogramTimeInterval(range),
          eventType: "dns",
          dnsType: "query",
          queryString: this.queryString,
        })
        .then((response: any) => {
          this.buildChart(response.data);
        });
    });
  }

  private buildChart(response: any[]): void {
    const values = [];
    const labels = [];
    response.forEach((e) => {
      labels.push(moment(e.key).toDate());
      values.push(e.count);
    });
    const ctx = getCanvasElementById("eventsOverTimeChart");
    const config: ChartConfiguration = {
      type: "bar",
      data: {
        labels: labels,
        datasets: [
          {
            data: values,
            backgroundColor: getColourPalette(values.length),
          },
        ],
      },
      options: {
        plugins: {
          title: {
            display: true,
            text: "DNS Requests Over Time",
            padding: 0,
          },
          legend: {
            display: false,
          },
        },
        scales: {
          x: {
            type: "time",
          },
        },
      },
    };
    if (this.charts.eventsOverTime) {
      this.charts.eventsOverTime.destroy();
    }
    this.charts.eventsOverTime = new Chart(ctx, config);
  }
}
