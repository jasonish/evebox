// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  createEffect,
  createMemo,
  createSignal,
  For,
  JSX,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { Top, TIME_RANGE, SET_TIME_RANGE } from "./Top";
import { Col, Container, Row } from "solid-bootstrap";
import { timeRangeAsSeconds } from "./settings";
import { statsAggBySensor } from "./api";
import { Chart } from "chart.js";
import { parse_timestamp } from "./datetime";
import dayjs from "dayjs";
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
  const [searchParams, setSearchParams] = useSearchParams<{
    sensor: string;
    min_timestamp: string;
    max_timestamp: string;
  }>();
  const [loadingCounter, setLoadingCounter] = createSignal(0);
  const [timeRange, setTimeRange] = createSignal<{
    min: string;
    max: string;
  } | null>(null);

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

  // React to URL parameter changes and time range changes
  createEffect(() => {
    // Track these reactive values to trigger re-render
    const sensor = searchParams.sensor;
    const minTs = searchParams.min_timestamp;
    const maxTs = searchParams.max_timestamp;
    const tr = TIME_RANGE();

    refresh();
  });

  // Clear timestamps when time range selector changes
  let previousTimeRange: string | undefined;
  createEffect(() => {
    const currentTimeRange = TIME_RANGE();

    // If time range changed and we had a previous value, clear timestamps
    if (
      previousTimeRange !== undefined &&
      previousTimeRange !== currentTimeRange
    ) {
      setSearchParams({
        min_timestamp: undefined,
        max_timestamp: undefined,
      });
    }

    previousTimeRange = currentTimeRange;
  });

  function destroyAllCharts() {
    console.log("Destroying charts...");
    while (charts.length > 0) {
      const chart = charts.pop();
      chart.destroy();
    }
  }

  // Use createMemo for computed values that depend on reactive state
  const timeWindow = createMemo<{ min: string; max: string } | null>(() => {
    const timeRangeSeconds = timeRangeAsSeconds();
    if (!timeRangeSeconds) return null;

    // If URL has timestamps, use them
    if (searchParams.min_timestamp && searchParams.max_timestamp) {
      return {
        min: searchParams.min_timestamp,
        max: searchParams.max_timestamp,
      };
    }

    // Otherwise calculate current time window based on NOW
    const now = dayjs();
    const startTime = now.subtract(timeRangeSeconds, "second");

    return {
      min: startTime.utc().toISOString(),
      max: now.utc().toISOString(),
    };
  });

  // Simplified navigation functions
  function navigateToPrevious() {
    const tw = timeWindow();
    if (!tw) return;

    const timeRangeSeconds = timeRangeAsSeconds();
    if (!timeRangeSeconds) return;

    // Move window back by one time range
    const minDate = dayjs(tw.min).subtract(timeRangeSeconds, "second");
    const maxDate = dayjs(tw.max).subtract(timeRangeSeconds, "second");

    setSearchParams({
      min_timestamp: minDate.utc().toISOString(),
      max_timestamp: maxDate.utc().toISOString(),
    });
  }

  function navigateToNext() {
    const tw = timeWindow();
    if (!tw) return;

    const timeRangeSeconds = timeRangeAsSeconds();
    if (!timeRangeSeconds) return;

    // Move window forward by one time range
    const minDate = dayjs(tw.min).add(timeRangeSeconds, "second");
    const maxDate = dayjs(tw.max).add(timeRangeSeconds, "second");

    setSearchParams({
      min_timestamp: minDate.utc().toISOString(),
      max_timestamp: maxDate.utc().toISOString(),
    });
  }

  function navigateToNow() {
    // Clear timestamps to show current time window
    setSearchParams({ min_timestamp: undefined, max_timestamp: undefined });
  }

  // Memoized computed values for button states
  const isViewingCurrentTime = createMemo(() => {
    return !searchParams.min_timestamp && !searchParams.max_timestamp;
  });

  const canNavigateNext = createMemo(() => {
    // Can't navigate next if we're already at current time
    if (isViewingCurrentTime()) return false;

    // Can navigate next if we have timestamps and max is in the past
    const tw = timeWindow();
    if (tw && tw.max) {
      const maxDate = dayjs(tw.max);
      const now = dayjs();
      // Allow navigation if the window's max time is before now
      return maxDate.isBefore(now);
    }

    return false;
  });

  function refresh() {
    const tw = timeWindow();
    loadData(searchParams.sensor, tw);
  }

  function loadData(
    selectedSensor: string | undefined,
    timeWindow: { min: string; max: string } | null,
  ) {
    destroyAllCharts();

    console.log("Loading charts...");

    for (let i = 0; i < CHARTS.length; i++) {
      const chart = CHARTS[i];
      loadingTracker(setLoadingCounter, () => {
        return statsAggBySensor(
          chart.field,
          chart.differential,
          timeWindow?.min,
          timeWindow?.max,
        ).then((response) => {
          // Capture time range from first response
          if (i === 0 && response.min_timestamp && response.max_timestamp) {
            setTimeRange({
              min: response.min_timestamp,
              max: response.max_timestamp,
            });
          }

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
        <Show when={timeRange()}>
          <Row class={"mt-2"}>
            <Col>
              <div
                class={"d-flex justify-content-end align-items-center gap-2"}
              >
                <div class={"text-muted small"}>
                  Showing data from{" "}
                  {parse_timestamp(timeRange()!.min).format(
                    "YYYY-MM-DD HH:mm:ss",
                  )}{" "}
                  to{" "}
                  {parse_timestamp(timeRange()!.max).format(
                    "YYYY-MM-DD HH:mm:ss",
                  )}
                </div>
                <div class={"btn-group"}>
                  <button
                    type={"button"}
                    class={"btn btn-sm btn-outline-secondary"}
                    onClick={navigateToPrevious}
                  >
                    &larr; Previous
                  </button>
                  <button
                    type={"button"}
                    class={"btn btn-sm btn-outline-secondary"}
                    onClick={navigateToNext}
                    disabled={!canNavigateNext()}
                  >
                    Next &rarr;
                  </button>
                  <button
                    type={"button"}
                    class={"btn btn-sm btn-outline-secondary"}
                    onClick={navigateToNow}
                    disabled={isViewingCurrentTime()}
                  >
                    Now
                  </button>
                </div>
              </div>
            </Col>
          </Row>
        </Show>
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
