// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  createEffect,
  createSignal,
  createUniqueId,
  onCleanup,
  Show,
} from "solid-js";
import { API, AggRequest } from "../api";
import { TIME_RANGE, Top } from "../Top";
import { Card, Col, Container, Row } from "solid-bootstrap";
import { Chart, ChartConfiguration } from "chart.js";
import { RefreshButton } from "../common/RefreshButton";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { Colors } from "../common/colors";
import { getChartCanvasElement, loadingTracker } from "../util";
import { createStore } from "solid-js/store";
import { CountValueDataTable } from "../components";
import dayjs from "dayjs";

const initialData = {
  topAlertsLoading: false,
  topAlerts: [],

  topDnsRequestsLoading: false,
  topDnsRequests: [],
  topDnsRequestsFrom: null,

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
  const [version, setVersion] = createSignal(0);
  const [loading, setLoading] = createSignal(0);
  let histogram: any = undefined;
  let hiddenTypes: { [key: string]: boolean } = {
    anomaly: true,
    stats: true,
    netflow: true,
  };
  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
  }>();
  const [data, setData] = createStore(initialData);

  const [eventsOverTimeLoading, setEventsOverTimeLoading] = createSignal(0);

  const [protocols, setProtocols] = createStore({
    loading: false,
    data: [],
  });
  let protocolsPieChartRef;

  function initChart() {
    if (histogram) {
      histogram.destroy();
    }
    buildChart();
  }

  onCleanup(() => {
    API.cancelAllSse();
  });

  createEffect(() => {
    refresh();
  });

  async function refresh() {
    setVersion((version) => version + 1);

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

      API.getSseAgg(request, version, (data: any) => {
        if (data === null) {
          setData("topAlertsLoading", false);
        } else {
          setData("topAlerts", data.rows);
        }
      });
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

      return API.getSseAgg(request, version, (data: any) => {
        if (data === null) {
          setData("topDnsRequestsLoading", false);
        } else {
          setData("topDnsRequestsFrom", data.earliest_ts);
          setData("topDnsRequests", data.rows);
        }
      });
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

      setProtocols("loading", true);
      setProtocols("data", []);

      return await API.getSseAgg(request, version, (data: any) => {
        if (data) {
          if (protocols.data.length == 0) {
            setProtocols("data", data.rows);
          } else {
            let labels = data.rows.map((e: any) => e.key);
            let dataset = data.rows.map((e: any) => e.count);
            let chart: any = Chart.getChart(protocolsPieChartRef!);
            chart.data.labels = labels;
            chart.data.datasets[0].data = dataset;
            chart.update();
          }
        }
      }).finally(() => {
        setProtocols("loading", false);
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

      return await API.getSseAgg(request, version, (data: any) => {
        if (data) {
          setData("tlsSni", {
            data: data.rows,
          });
        }
      }).finally(() => {
        setData("tlsSni", {
          loading: false,
        });
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

      return await API.getSseAgg(request, version, (data: any) => {
        if (data) {
          setData("quicSni", {
            data: data.rows,
          });
        }
      }).finally(() => {
        setData("quicSni", {
          loading: false,
        });
      });
    });

    fetchEventsHistogram(q);
  }

  async function fetchEventsHistogram(q: string) {
    initChart();

    let eventTypes = await API.getEventTypes({
      time_range: TIME_RANGE(),
    });

    let labels: number[] = [];

    for (const row of eventTypes) {
      let request = {
        time_range: TIME_RANGE(),
        event_type: row,
        query_string: q,
      };

      loadingTracker(setLoading, async () => {
        setEventsOverTimeLoading((v) => v + 1);
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
          let hidden = hiddenTypes[row];
          let colorIdx = histogram.data.datasets.length;
          histogram.data.datasets.push({
            data: values,
            label: row,
            pointRadius: 0,
            hidden: hidden,
            backgroundColor: Colors[colorIdx % Colors.length],
            borderColor: Colors[colorIdx % Colors.length],
          });
          histogram.update();
        }
      }).finally(() => {
        setEventsOverTimeLoading((v) => v - 1);
      });
    }
  }

  function buildChart() {
    const ctx = getChartCanvasElement("histogram");

    const config: ChartConfiguration | any = {
      type: "bar",
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
          tooltip: {
            enabled: true,
            callbacks: {
              label: function (context: any) {
                let label = context.dataset.label;
                let value = context.parsed.y;
                if (value == 0) {
                  return null;
                }
                return `${label}: ${value}`;
              },
            },
            // Sort items in descending order.
            itemSort: function (a: any, b: any) {
              return b.raw - a.raw;
            },
            // Limit the tooltip to the top 5 items. Like default Kibana.
            filter: function (item: any, _data: any) {
              return item.datasetIndex < 6;
            },
          },
          legend: {
            display: true,
            position: "top",
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
            stacked: true,
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
              <Card.Header class="d-flex">
                <b>Events by Type Over Time</b>
                <Show when={eventsOverTimeLoading() > 0}>
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
                  when={protocols.loading !== undefined && protocols.loading}
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
                <PieChart data={protocols.data} ref={protocolsPieChartRef} />
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
              title="Top DNS Reqeuests"
              suffix={() => {
                if (data.topDnsRequestsFrom) {
                  const timestamp = dayjs(data.topDnsRequestsFrom);
                  return `from ${timestamp.fromNow()}`;
                }
              }}
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

function PieChart(props: { data: any[]; ref?: any }) {
  const chartId = createUniqueId();
  let chart: any = null;

  createEffect(() => {
    const element = getChartCanvasElement(chartId);

    if (chart != null) {
      chart.destroy();
    }

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
            ref={props.ref}
            id={chartId}
            style="max-height: 150px; height: 150px;"
          ></canvas>
        </div>
      </div>
    </>
  );
}
