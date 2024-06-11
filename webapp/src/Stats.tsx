// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createEffect, createSignal, For, JSX, onCleanup } from "solid-js";
import { Top } from "./Top";
import { Col, Container, Row } from "solid-bootstrap";
import { timeRangeAsSeconds } from "./settings";
import { statsAgg } from "./api";
import { Chart } from "chart.js";
import { parse_timestamp } from "./datetime";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "./common/SensorSelect";
import { RefreshButton } from "./common/RefreshButton";
import { loadingTracker } from "./util";

interface ChartConfig {
  title: string;
  field: string;
  differential: boolean;
  canvasId: string;
}

const CHARTS: ChartConfig[] = [
  {
    title: "Decoder Bytes",
    field: "stats.decoder.bytes",
    differential: true,
    canvasId: "decoderBytes",
  },
  {
    title: "Decoder Packets",
    field: "stats.decoder.pkts",
    differential: true,
    canvasId: "decoderPackets",
  },
  {
    title: "Kernel Drops",
    field: "stats.capture.kernel_drops",
    differential: true,
    canvasId: "kernelDrops",
  },
  {
    title: "Flow Memory",
    field: "stats.flow.memuse",
    differential: false,
    canvasId: "flowMemuse",
  },
  {
    title: "TCP Memory",
    field: "stats.tcp.memuse",
    differential: false,
    canvasId: "tcpMemuse",
  },
];

export function Stats(): JSX.Element {
  const [searchParams, setSearchParams] = useSearchParams<{ sensor: string }>();
  const [loadingCounter, setLoadingCounter] = createSignal(0);
  let charts: any[] = [];

  onCleanup(() => {
    destroyAllCharts();
  });

  createEffect(() => {
    refresh();
  });

  function destroyAllCharts() {
    console.log("Destroying charts...");
    while (charts.length > 0) {
      const chart = charts.pop();
      chart.destroy();
    }
  }

  function refresh() {
    loadData(timeRangeAsSeconds(), searchParams.sensor);
  }

  function loadData(timeRange: undefined | number, sensor: string | undefined) {
    destroyAllCharts();

    console.log("Loading chart...");

    for (const chart of CHARTS) {
      loadingTracker(setLoadingCounter, () => {
        return statsAgg(
          chart.field,
          chart.differential,
          timeRange,
          sensor
        ).then((response) => {
          const labels: any[] = [];
          const values: any[] = [];
          response.data.forEach((e) => {
            labels.push(parse_timestamp(e.timestamp).toDate());
            values.push(e.value);
          });
          const canvas = buildChart(
            chart.canvasId,
            chart.title,
            labels,
            values
          );
          charts.push(canvas);
        });
      });
    }
  }

  return (
    <div>
      <Top />
      <Container fluid>
        <Row class={"mt-2"}>
          <Col>
            <form class={"row row-cols-lg-auto align-items-center"}>
              <div class={"col-12"}>
                <RefreshButton loading={loadingCounter()} refresh={refresh} />
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
        <For each={CHARTS}>
          {(chart) => (
            <>
              <Row>
                <Col>
                  <div style={"height: 200px; width: 100%"}>
                    <canvas id={chart.canvasId}></canvas>
                  </div>
                </Col>
              </Row>
            </>
          )}
        </For>
      </Container>
    </div>
  );
}

function buildChart(
  elementId: string,
  title: string,
  labels: Date[],
  values: number[]
): Chart<any> {
  const ctx = (
    document.getElementById(elementId) as HTMLCanvasElement
  ).getContext("2d") as CanvasRenderingContext2D;

  const chart = new Chart(ctx, {
    type: "line",
    data: {
      labels: labels,
      datasets: [
        {
          data: values,
          backgroundColor: "rgba(0, 90, 0, 0.3)",
          borderColor: "rgba(0, 90, 0, 1)",
          pointRadius: 0,
          fill: true,
          borderWidth: 1,
        },
      ],
    },
    options: {
      interaction: {
        intersect: false,
      },
      responsive: true,
      maintainAspectRatio: false,
      scales: {
        x: {
          type: "time",
        },
      },
      plugins: {
        title: {
          text: title,
          display: true,
        },
        legend: {
          display: false,
        },
      },
    },
  });
  return chart;
}
