// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal } from "solid-js";
import { Col, Container, Form, Row } from "solid-bootstrap";
import { API, fetchAgg } from "../api";
import { RefreshButton } from "../common/RefreshButton";
import { Chart, ChartConfiguration } from "chart.js";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { loadingTracker } from "../util";
import { CountValueDataTable } from "../components/CountValueDataTable";
import { Colors } from "../common/colors";

interface CountValueRow {
  count: number;
  key: any;
}

export function AlertsReport() {
  const [mostAlerts, setMostAlerts] = createSignal<CountValueRow[]>([]);
  const [leastAlerts, setLeastAlerts] = createSignal<CountValueRow[]>([]);
  const [mostSourceAddrs, setMostSourceAddrs] = createSignal<CountValueRow[]>(
    []
  );
  const [mostDestAddrs, setMostDestAddrs] = createSignal<CountValueRow[]>([]);
  const [leastSourceAddrs, setLeastSourcesAddrs] = createSignal<
    CountValueRow[]
  >([]);
  const [leastDestAddrs, setLeastDestAddrs] = createSignal<CountValueRow[]>([]);
  const [loading, setLoading] = createSignal(0);
  const [searchParams, setSearchParams] = useSearchParams();

  let histogram: any = undefined;

  createEffect(() => {
    refresh();
  });

  function buildChart(response: any) {
    const dataValues: number[] = [];
    const dataLabels: number[] = [];
    response.data.forEach((e: any) => {
      dataValues.push(e.count);
      dataLabels.push(e.time);
    });

    const ctx = (
      document.getElementById("histogram") as HTMLCanvasElement
    ).getContext("2d") as CanvasRenderingContext2D;

    const config: ChartConfiguration = {
      type: "bar",
      data: {
        labels: dataLabels,
        datasets: [
          {
            data: dataValues,

            backgroundColor: dataValues.map(
              (_, index) => Colors[index % Colors.length]
            ),
            borderColor: dataValues.map(
              (_, index) => Colors[index % Colors.length]
            ),
          },
        ],
      },
      options: {
        plugins: {
          title: {
            display: true,
            text: "Alerts Over Time",
            padding: 0,
          },
          legend: {
            display: false,
          },
        },
        scales: {
          x: {
            type: "time",
            ticks: {
              source: "auto",
            },
          },
        },
      },
    };
    if (histogram) {
      histogram.destroy();
    }
    histogram = new Chart(ctx, config);
  }

  function refresh() {
    console.log("AlertsReport.refresh");
    const timeRange = TIME_RANGE();

    let queryString = [];
    if (searchParams.q) {
      queryString.push(searchParams.q);
    }
    if (searchParams.sensor) {
      queryString.push(`host:${searchParams.sensor}`);
    }

    API.histogramTime({
      time_range: timeRange,
      event_type: "alert",
      query_string: queryString.length > 0 ? queryString.join(" ") : undefined,
    }).then((response) => {
      buildChart(response);
    });

    let loaders: {
      field: string;
      q: string;
      order: "desc" | "asc";
      setter: (arg0: any) => void;
    }[] = [
      // Top alerting signatures.
      {
        field: "alert.signature",
        q: "event_type:alert",
        order: "desc",
        setter: setMostAlerts,
      },
      // Least alerting signatures.
      {
        field: "alert.signature",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastAlerts,
      },
      // Top alerting source addresses.
      {
        field: "src_ip",
        q: "event_type:alert",
        order: "desc",
        setter: setMostSourceAddrs,
      },
      // Top alerting destination addresses.
      {
        field: "dest_ip",
        q: "event_type:alert",
        order: "desc",
        setter: setMostDestAddrs,
      },
      {
        field: "src_ip",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastSourcesAddrs,
      },
      {
        field: "dest_ip",
        q: "event_type:alert",
        order: "asc",
        setter: setLeastDestAddrs,
      },
    ];

    for (const loader of loaders) {
      let q = [...queryString];
      if (loader.q) {
        q.push(loader.q);
      }

      let request = {
        time_range: timeRange,
        field: loader.field,
        order: loader.order,
        q: q.length > 0 ? q.join(" ") : undefined,
      };

      loader.setter([]);

      loadingTracker(setLoading, () => {
        return fetchAgg(request).then((response) => {
          loader.setter(response.rows);
        });
      });
    }
  }

  return (
    <>
      <Top />

      <Container fluid={true}>
        <Row class="mt-2">
          <Col>
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

          <Col>
            <Form
              class="input-group"
              onsubmit={(e) => {
                e.preventDefault();
                // blurInputs();
                // applyFilter(e.currentTarget.filter.value);
                setSearchParams({ q: e.currentTarget.filter.value });
              }}
            >
              <input
                id="filter-input"
                type="text"
                class="form-control"
                name="filter"
                placeholder="Search..."
                value={searchParams.q || ""}
                onkeydown={(e) => {
                  if (
                    e.code === "Escape" ||
                    e.key === "Escape" ||
                    e.keyCode === 27
                  ) {
                    // blurInputs();
                  }
                  e.stopPropagation();
                }}
              />
              <button class="btn btn-secondary" type="submit">
                Apply
              </button>
              <button
                class="btn btn-secondary"
                type="button"
                onclick={() => {
                  setSearchParams({ q: undefined });
                }}
              >
                Clear
              </button>
            </Form>
          </Col>
        </Row>

        <Row>
          <Col class={"mt-2"}>
            <canvas
              id={"histogram"}
              style="max-height: 250px; height: 300px"
            ></canvas>
          </Col>
        </Row>

        <Row class={"mt-2"}>
          <Col>
            <CountValueDataTable
              title={"Most Alerting Signatures"}
              label={"Signature"}
              searchField="alert.signature"
              rows={mostAlerts()}
            />
          </Col>

          <Col>
            <CountValueDataTable
              title={"Least Alerting Signatures"}
              label={"Signature"}
              searchField="alert.signature"
              rows={leastAlerts()}
            />
          </Col>
        </Row>

        <Row>
          <Col class={"mt-2"}>
            <CountValueDataTable
              title={"Most Alerting Source Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={mostSourceAddrs()}
            />
          </Col>
          <Col class={"mt-2"}>
            <CountValueDataTable
              title={"Most Alerting Destination Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={mostDestAddrs()}
            />
          </Col>
        </Row>

        <Row>
          <Col class={"mt-2"}>
            <CountValueDataTable
              title={"Least Alerting Source Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={leastSourceAddrs()}
            />
          </Col>
          <Col class={"mt-2"}>
            <CountValueDataTable
              title={"Least Alerting Destination Addresses"}
              label={"Address"}
              searchField={"@ip"}
              rows={leastDestAddrs()}
            />
          </Col>
        </Row>
      </Container>
    </>
  );
}
