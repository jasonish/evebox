// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createEffect, createSignal } from "solid-js";
import { API, AggRequest, fetchAgg } from "../api";
import { TIME_RANGE, Top } from "../Top";
import { Card, Col, Container, Row } from "solid-bootstrap";
import { Chart, ChartConfiguration } from "chart.js";
import { RefreshButton } from "../common/RefreshButton";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { Colors } from "../common/colors";
import { loadingTracker } from "../util";
import { createStore } from "solid-js/store";
import { CountValueDataTable } from "../components/CountValueDataTable";

const initialData = {
  topAlertsLoading: false,
  topAlerts: [],
  topDnsRequestsLoading: false,
  topDnsRequests: [],
};

export function OverviewReport() {
  const [loading, setLoading] = createSignal(0);
  let histogram: any = undefined;
  let hiddenTypes: { [key: string]: boolean } = {
    anomaly: true,
    stats: true,
    netflow: true,
  };
  const [searchParams, setSearchParams] = useSearchParams();
  const [data, setData] = createStore(initialData);

  function initChart() {
    if (histogram) {
      histogram.destroy();
    }
    buildChart();
  }

  createEffect(() => {
    refresh();
  });

  async function refresh() {
    let q = "";
    if (searchParams.sensor) {
      q += `host:${searchParams.sensor}`;
    }

    loadingTracker(setLoading, async () => {
      let request: AggRequest = {
        field: "alert.signature",
        size: 10,
        order: "desc",
        time_range: TIME_RANGE(),
        q: q,
      };
      setData("topAlertsLoading", true);
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      setData("topAlerts", response.rows);
      setData("topAlertsLoading", false);
    });

    loadingTracker(setLoading, async () => {
      let request: AggRequest = {
        field: "dns.rrname",
        size: 10,
        order: "desc",
        time_range: TIME_RANGE(),
        q: q + " dns.type:query",
      };
      setData("topDnsRequestsLoading", true);
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      setData("topDnsRequests", response.rows);
      setData("topDnsRequestsLoading", false);
    });

    fetchEventsHistogram(q);
  }

  async function fetchEventsHistogram(q: string) {
    console.log("Fetching histogram");

    initChart();

    let eventTypeAggs = await loadingTracker(setLoading, () =>
      fetchAgg({
        field: "event_type",
        size: 100,
        time_range: TIME_RANGE(),
        q: q,
      }).then((response) => response.rows)
    );

    let eventTypes: string[] = [];
    let labels: number[] = [];

    for (const row of eventTypeAggs) {
      const eventType = row.key;
      eventTypes.push(eventType);
      let request = {
        time_range: TIME_RANGE(),
        event_type: eventType,
        query_string: q,
      };

      loadingTracker(setLoading, async () => {
        let response = await API.histogramTime(request);
        if (labels.length === 0) {
          response.data.forEach((e) => {
            labels.push(e.time);
          });
          histogram.data.labels = labels;
        }

        if (response.data.length != labels.length) {
          console.log("ERROR: Label and data mismatch");
        } else {
          let values = response.data.map((e) => e.count);
          let hidden = hiddenTypes[eventType];
          let colorIdx = histogram.data.datasets.length;
          histogram.data.datasets.push({
            data: values,
            label: row.key,
            pointRadius: 0,
            hidden: hidden,
            backgroundColor: Colors[colorIdx % Colors.length],
            borderColor: Colors[colorIdx % Colors.length],
          });
          histogram.update();
        }
      });
    }
  }

  function buildChart() {
    const ctx = (
      document.getElementById("histogram") as HTMLCanvasElement
    ).getContext("2d") as CanvasRenderingContext2D;

    const config: ChartConfiguration | any = {
      type: "line",
      data: {
        labels: [],
        datasets: [],
      },
      options: {
        plugins: {
          title: {
            display: false,
            padding: 0,
          },
          legend: {
            display: true,
            onClick: (_e: any, legendItem: any, legend: any) => {
              const eventType = legendItem.text;
              const index = legendItem.datasetIndex;
              const ci = legend.chart;
              if (ci.isDatasetVisible(index)) {
                ci.hide(index);
                legendItem.hidden = true;
                hiddenTypes[eventType] = true;
              } else {
                ci.show(index);
                legendItem.hidden = false;
                hiddenTypes[eventType] = false;
              }
            },
          },
        },
        interaction: {
          intersect: false,
          mode: "nearest",
          axis: "x",
        },
        elements: {
          line: {
            tension: 0.4,
          },
        },
        scales: {
          x: {
            type: "time",
            ticks: {
              source: "auto",
            },
          },
          y: {
            display: true,
          },
        },
      },
    };
    if (histogram) {
      histogram.destroy();
    }
    histogram = new Chart(ctx, config);
  }

  return (
    <>
      <Top />
      <Container fluid>
        <Row>
          <Col class={"mt-2"}>
            <form class={"row row-cols-lg-auto align-items-center"}>
              <div class={"col-12"}>
                <RefreshButton
                  loading={loading()}
                  refresh={refresh}
                  showProgress={true}
                />
              </div>
              <div class={"col-12"}>
                <SensorSelect
                  selected={searchParams.sensor}
                  onchange={(sensor) => {
                    setSearchParams({ sensor: sensor });
                  }}
                />
              </div>
            </form>
          </Col>
        </Row>
        <Row>
          <Col class={"mt-2"}>
            <Card>
              <Card.Header class={"text-center"}>
                <b>Events by Type Over Time</b>
              </Card.Header>
              <Card.Body class={"p-0"}>
                <canvas
                  id={"histogram"}
                  style="max-height: 250px; height: 300px"
                ></canvas>
              </Card.Body>
            </Card>
          </Col>
        </Row>

        <div class="row mt-2">
          <div class="col">
            <CountValueDataTable
              title="Top Alerts"
              label="Signature"
              rows={data.topAlerts}
              loading={data.topAlertsLoading}
              searchField="alert.signature"
            />
          </div>
          <div class="col">
            <CountValueDataTable
              title="Top DNS Requests"
              label="Hostname"
              rows={data.topDnsRequests}
              loading={data.topDnsRequestsLoading}
              searchField="dns.rrname"
            />
          </div>
        </div>
      </Container>
    </>
  );
}
