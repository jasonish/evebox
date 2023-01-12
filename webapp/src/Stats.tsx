// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import {
  createEffect,
  createSignal,
  For,
  JSX,
  onCleanup,
  onMount,
} from "solid-js";
import { Top } from "./Top";
import { Button, Col, Container, Row } from "solid-bootstrap";
import { timeRangeAsSeconds } from "./settings";
import { getSensors, statsAgg } from "./api";
import { Chart } from "chart.js";
import { parse_timestamp } from "./datetime";
import "chartjs-adapter-dayjs-4";
import { useSearchParams } from "@solidjs/router";

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
  const [sensors, setSensors] = createSignal<string[]>([]);
  const [searchParams, setSearchParams] = useSearchParams<{ sensor: string }>();
  let charts: any[] = [];

  onCleanup(() => {
    destroyAllCharts();
  });

  createEffect(() => {
    loadData(timeRangeAsSeconds(), searchParams.sensor);
  });

  function destroyAllCharts() {
    console.log("Destroying charts...");
    while (charts.length > 0) {
      const chart = charts.pop();
      chart.destroy();
    }
  }

  function loadData(timeRange: undefined | number, sensor: string | undefined) {
    destroyAllCharts();

    getSensors().then((response) => {
      setSensors(response.data);
    });

    console.log("Loading chart...");

    for (const chart of CHARTS) {
      statsAgg(chart.field, chart.differential, timeRange, sensor).then(
        (response) => {
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
        }
      );
    }
  }

  return (
    <div>
      <Top />
      <Container fluid>
        <Row class={"mt-2"}>
          <Col sm={1}>
            <Button variant={"secondary"}>Refresh</Button>
          </Col>
          <Col>
            <div class={"row align-items-center"}>
              <label for={"event-type-selector"} class={"col-auto"}>
                Sensor:
              </label>
              <div class={"col-auto"}>
                <select
                  class="form-select"
                  id={"event-type-selector"}
                  onchange={(e) => {
                    setSearchParams({ sensor: e.currentTarget.value });
                    e.currentTarget.blur();
                  }}
                >
                  <option value="" selected={searchParams.sensor == undefined}>
                    All
                  </option>
                  <For each={sensors()}>
                    {(sensor) => {
                      return (
                        <>
                          <option
                            value={sensor}
                            selected={sensor == searchParams.sensor}
                          >
                            {sensor}
                          </option>
                        </>
                      );
                    }}
                  </For>
                </select>
              </div>
            </div>
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
