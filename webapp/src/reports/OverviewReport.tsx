// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createEffect, createSignal, untrack } from "solid-js";
import { API } from "../api";
import { TIME_RANGE, Top } from "../Top";
import { Card, Col, Container, Row } from "solid-bootstrap";
import { Chart, ChartConfiguration } from "chart.js";
import { RefreshButton } from "../common/RefreshButton";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";

export function OverviewReport() {
  const [loading, setLoading] = createSignal(0);
  let histogram: any = undefined;
  let hiddenTypes: { [key: string]: boolean } = {
    anomaly: true,
    stats: true,
    netflow: true,
  };
  const [searchParams, setSearchParams] = useSearchParams();

  createEffect(() => {
    refresh();
  });

  function initChart() {
    if (histogram) {
      histogram.destroy();
    }
    buildChart();
  }

  function refresh() {
    untrack(() => {
      if (loading() > 0) {
        return;
      }
    });

    let q: string[] = [];

    if (searchParams.sensor) {
      q.push(`host:${searchParams.sensor}`);
    }

    setLoading((n) => n + 1);
    initChart();
    let timeRange = TIME_RANGE();
    API.groupBy({
      field: "event_type",
      size: 100,
      time_range: timeRange,
      q: q.length > 0 ? q.join(" ") : undefined,
    })
      .then((response) => {
        let eventTypes: string[] = [];
        let labels: number[] = [];
        for (const e of response.rows) {
          let eventType = e.key;
          eventTypes.push(eventType);
          setLoading((n) => n + 1);
          API.histogramTime({
            time_range: TIME_RANGE(),
            event_type: e.key,
            query_string: q.length > 0 ? q.join(" ") : undefined,
          })
            .then((response) => {
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
                histogram.data.datasets.push({
                  data: values,
                  label: e.key,
                  pointRadius: 0,
                  hidden: hidden,
                });
                histogram.update();
              }
            })
            .finally(() => {
              setLoading((n) => n - 1);
            });
        }
      })
      .finally(() => {
        setLoading((n) => n - 1);
      });
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
          colors: {
            // Required to use color on dynamic charts.
            forceOverride: true,
          },
          title: {
            display: false,
            padding: 0,
          },
          legend: {
            display: true,
            onClick: (e: any, legendItem: any, legend: any) => {
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
      </Container>
    </>
  );
}
