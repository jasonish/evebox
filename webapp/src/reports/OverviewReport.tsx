// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createEffect, createSignal, createUniqueId, Show } from "solid-js";
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

  protocols: {
    loading: false,
    data: [],
  },

  tlsSni: {
    loading: false,
    data: [],
  },

  quicSni: {
    loading: false,
    data: [],
  },
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
        q: q + " event_type:alert",
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
        q: q + " event_type:dns dns.type:query",
      };
      setData("topDnsRequestsLoading", true);
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      setData("topDnsRequests", response.rows);
      setData("topDnsRequestsLoading", false);
    });

    loadingTracker(setLoading, async () => {
      let request: AggRequest = {
        field: "proto",
        size: 10,
        time_range: TIME_RANGE(),

        // Limit to flow types to get an accurate count, otherwise
        // we'll get duplicate counts from different event types.
        q: q + " event_type:flow",
      };
      setData("protocols", {
        loading: true,
      });
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      console.log(response.rows);
      setData("protocols", {
        loading: false,
        data: response.rows,
      });
    });

    // TLS SNI.
    loadingTracker(setLoading, async () => {
      let request: AggRequest = {
        field: "tls.sni",
        size: 10,
        time_range: TIME_RANGE(),
        q: q + " event_type:tls",
      };
      setData("tlsSni", {
        loading: true,
      });
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      console.log(response.rows);
      setData("tlsSni", {
        loading: false,
        data: response.rows,
      });
    });

    // Quic SNI.
    loadingTracker(setLoading, async () => {
      let request: AggRequest = {
        field: "quic.sni",
        size: 10,
        time_range: TIME_RANGE(),
        q: q + " event_type:quic",
      };
      setData("quicSni", {
        loading: true,
      });
      let response = await loadingTracker(setLoading, () => fetchAgg(request));
      console.log(response.rows);
      setData("quicSni", {
        loading: false,
        data: response.rows,
      });
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
        responsive: true,
        maintainAspectRatio: false,

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
                <RefreshButton loading={loading()} refresh={refresh} />
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

        <div class="row">
          <div class="mt-2 col col-lg-10 col-md-8 col-sm-12">
            <Card>
              <Card.Header class={"text-center"}>
                <b>Events by Type Over Time</b>
              </Card.Header>
              <Card.Body class={"p-0"}>
                <div class="chart-container" style="position; relative;">
                  <canvas
                    id={"histogram"}
                    style="max-height: 180px; height: 180px;"
                  ></canvas>
                </div>
              </Card.Body>
            </Card>
          </div>
          <div class="mt-2 col col-lg-2 col-md-4 col-sm-12">
            <div class="card">
              <div class="card-header d-flex">
                Protocols
                <Show
                  when={
                    data.protocols.loading !== undefined &&
                    data.protocols.loading
                  }
                >
                  {/* Loader in a button for placement reason's. */}
                  <button
                    class="btn ms-auto"
                    type="button"
                    disabled
                    style="border: 0; padding: 0;"
                  >
                    <span
                      class="spinner-border spinner-border-sm"
                      aria-hidden="true"
                    ></span>
                    <span class="visually-hidden" role="status">
                      Loading...
                    </span>
                  </button>
                </Show>
              </div>
              <div class="card-body p-0">
                <PieChart data={data.protocols.data} />
              </div>
            </div>
          </div>
        </div>

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

        <div class="row mt-2">
          <div class="col">
            <CountValueDataTable
              title="Top TLS SNI"
              label="Hostname"
              rows={data.tlsSni.data}
              loading={data.tlsSni.loading}
              searchField="tls.sni"
            />
          </div>
          <div class="col">
            <CountValueDataTable
              title="Top Quic SNI"
              label="Hostname"
              rows={data.quicSni.data}
              loading={data.quicSni.loading}
              searchField="quic.sni"
            />
          </div>
        </div>
      </Container>
    </>
  );
}

function PieChart(props: { data: any[] }) {
  const chartId = createUniqueId();
  let chart: any = null;

  createEffect(() => {
    const element = getChartElement(chartId);

    if (chart != null) {
      chart.destroy();
    }

    const values = props.data.map((e) => e.count);
    console.log(values);

    chart = new Chart(element, {
      type: "pie",
      data: {
        labels: props.data.map((e) => e.key),
        datasets: [
          {
            data: props.data.map((e) => e.count),
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            display: true,
            labels: {
              font: {
                size: 10,
              },
            },
            onHover: (_evt, legendItem) => {
              const activeElement = {
                datasetIndex: 0,
                index: legendItem.index,
              };
              chart.tooltip.setActiveElements([activeElement]);
              chart.update();
            },
          },
        },
      },
    });
  });

  return (
    <>
      <div>
        <div class="chart-container" style="height: 180px; position; relative;">
          <canvas
            id={chartId}
            style="max-height: 150px; height: 150px;"
          ></canvas>
        </div>
      </div>
    </>
  );
}

// Move to utils.
function getChartElement(id: string) {
  let element = document.getElementById(id) as HTMLCanvasElement;
  return element.getContext("2d") as CanvasRenderingContext2D;
}
