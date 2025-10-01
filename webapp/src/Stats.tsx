// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  createEffect,
  createSignal,
  For,
  JSX,
  onCleanup,
  onMount,
} from "solid-js";
import { Top, TIME_RANGE, SET_TIME_RANGE } from "./Top";
import { Col, Container, Row } from "solid-bootstrap";
import { timeRangeAsSeconds } from "./settings";
import { statsAggBySensor } from "./api";
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

// Define a color palette for different sensors
const SENSOR_COLORS = [
  { bg: "rgba(0, 90, 0, 0.3)", border: "rgba(0, 90, 0, 1)" },
  { bg: "rgba(90, 0, 0, 0.3)", border: "rgba(90, 0, 0, 1)" },
  { bg: "rgba(0, 0, 90, 0.3)", border: "rgba(0, 0, 90, 1)" },
  { bg: "rgba(90, 90, 0, 0.3)", border: "rgba(90, 90, 0, 1)" },
  { bg: "rgba(90, 0, 90, 0.3)", border: "rgba(90, 0, 90, 1)" },
  { bg: "rgba(0, 90, 90, 0.3)", border: "rgba(0, 90, 90, 1)" },
  { bg: "rgba(45, 45, 45, 0.3)", border: "rgba(45, 45, 45, 1)" },
  { bg: "rgba(135, 45, 0, 0.3)", border: "rgba(135, 45, 0, 1)" },
  { bg: "rgba(0, 135, 45, 0.3)", border: "rgba(0, 135, 45, 1)" },
  { bg: "rgba(45, 0, 135, 0.3)", border: "rgba(45, 0, 135, 1)" },
];

export function Stats(): JSX.Element {
  const [searchParams, setSearchParams] = useSearchParams<{ sensor: string }>();
  const [loadingCounter, setLoadingCounter] = createSignal(0);
  let charts: any[] = [];

  // Enforce max 7-day time range for stats page
  onMount(() => {
    if (TIME_RANGE() === "") {
      SET_TIME_RANGE("7d");
    }
  });

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

  function loadData(
    timeRange: undefined | number,
    selectedSensor: string | undefined,
  ) {
    destroyAllCharts();

    console.log("Loading charts...");

    for (const chart of CHARTS) {
      loadingTracker(setLoadingCounter, () => {
        return statsAggBySensor(
          chart.field,
          chart.differential,
          timeRange,
        ).then((response) => {
          // Build datasets for each sensor
          const datasets: any[] = [];
          let allTimestamps = new Set<string>();

          // Filter data if a specific sensor is selected
          let filteredData = response.data;
          if (selectedSensor) {
            // Handle both regular sensor names and "(no-name)"
            filteredData = Object.fromEntries(
              Object.entries(response.data).filter(
                ([sensor]) => sensor === selectedSensor,
              ),
            );
          }

          // First, collect all unique timestamps
          Object.entries(filteredData).forEach(([sensor, dataPoints]) => {
            dataPoints.forEach((dp) => {
              allTimestamps.add(dp.timestamp);
            });
          });

          // Sort timestamps
          const sortedTimestamps = Array.from(allTimestamps).sort();
          const labels = sortedTimestamps.map((ts) =>
            parse_timestamp(ts).toDate(),
          );

          // Create a dataset for each sensor
          let colorIndex = 0;
          Object.entries(filteredData).forEach(([sensor, dataPoints]) => {
            const color = SENSOR_COLORS[colorIndex % SENSOR_COLORS.length];
            colorIndex++;

            // Create a map for quick lookup
            const valueMap = new Map<string, number>();
            dataPoints.forEach((dp) => {
              valueMap.set(dp.timestamp, dp.value);
            });

            // Build values array aligned with all timestamps
            const values = sortedTimestamps.map((ts) => {
              return valueMap.get(ts) || 0;
            });

            datasets.push({
              label: sensor,
              data: values,
              backgroundColor: color.bg,
              borderColor: color.border,
              pointRadius: 0,
              fill: false,
              borderWidth: 2,
            });
          });

          const canvas = buildChart(
            chart.canvasId,
            chart.title,
            labels,
            datasets,
          );
          charts.push(canvas);
        });
      });
    }
  }

  return (
    <div>
      <Top excludeTimeRanges={[""]} />
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
  datasets: any[],
): Chart<any> {
  const ctx = (
    document.getElementById(elementId) as HTMLCanvasElement
  ).getContext("2d") as CanvasRenderingContext2D;

  const chart = new Chart(ctx, {
    type: "line",
    data: {
      labels: labels,
      datasets: datasets,
    },
    options: {
      interaction: {
        intersect: false,
        mode: "index",
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
          display: true,
          position: "top",
        },
        tooltip: {
          mode: "index",
          intersect: false,
        },
      },
    },
  });
  return chart;
}
