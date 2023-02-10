// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal, For, Show } from "solid-js";
import * as API from "../api";
import { Card, Col, Container, Row, Table } from "solid-bootstrap";
import { GroupByQueryRequest } from "../api";
import { RefreshButton } from "../common/RefreshButton";
import { Chart, ChartConfiguration } from "chart.js";
import { serverConfig } from "../config";
import { parse_timerange } from "../datetime";
import dayjs from "dayjs";

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
  const [loading, setLoading] = createSignal(0);

  let histogram: any = undefined;

  createEffect(() => {
    refresh(TIME_RANGE());
  });

  function forceRefresh() {
    console.log("AlertsReport.forceRefresh");
    refresh(TIME_RANGE());
  }

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
            time: {
              //unit: "minute",
            },
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

  function getInterval(rangeSeconds: number | undefined): string {
    let bounds = [
      { seconds: 60, interval: "1s" },
      { seconds: 3600 * 1, interval: "1m" },
      { seconds: 3600 * 3, interval: "2m" },
      { seconds: 3600 * 6, interval: "5m" },
      { seconds: 3600 * 12, interval: "15m" },
      { seconds: 3600 * 24, interval: "30m" },
      { seconds: 3600 * 24 * 3, interval: "2h" },
      { seconds: 3600 * 24 * 7, interval: "3h" },
    ];

    if (!rangeSeconds) {
      return "1d";
    } else {
      for (const bound of bounds) {
        if (rangeSeconds <= bound.seconds) {
          return bound.interval;
        }
      }
      return "1d";
    }
  }

  function refresh(timeRange: string) {
    const rangeSeconds = parse_timerange(timeRange);
    const interval = getInterval(rangeSeconds);

    API.histogram_time({
      time_range: timeRange,
      interval: interval,
      event_type: "alert",
    }).then((response) => {
      buildChart(response);
    });

    let loaders = [
      // Top alerting signatures.
      {
        request: {
          field: "alert.signature",
          q: "event_type:alert",
        },
        setter: setMostAlerts,
      },
      // Least alerting signatures.
      {
        request: {
          field: "alert.signature",
          q: "event_type:alert",
          order: "asc",
        },
        setter: setLeastAlerts,
      },
      // Top alerting source addresses.
      {
        request: {
          field: "src_ip",
          q: "event_type:alert",
          order: "desc",
        },
        setter: setMostSourceAddrs,
      },
      // Top alerting destination addresses.
      {
        request: {
          field: "dest_ip",
          q: "event_type:alert",
          order: "desc",
        },
        setter: setMostDestAddrs,
      },
    ];

    for (const loader of loaders) {
      let request = {
        time_range: timeRange,
        ...loader.request,
      } as GroupByQueryRequest;
      setLoading((n) => n + 1);
      loader.setter([]);
      API.groupBy(request)
        .then((response) => {
          loader.setter(response.rows);
        })
        .finally(() => {
          setLoading((n) => n - 1);
        });
    }
  }

  return (
    <>
      <Top />

      <Container fluid={true}>
        <Row class="mt-2">
          <Col>
            <RefreshButton loading={loading()} refresh={forceRefresh} />
          </Col>
        </Row>

        <Row>
          <Col>
            <canvas
              id={"histogram"}
              style="max-height: 250px; height: 300px"
            ></canvas>
          </Col>
        </Row>

        <Row class={"mt-2"}>
          <Col>
            <CountValueTable
              title={"Most Alerting Signatures"}
              label={"Signature"}
              rows={mostAlerts()}
            />
          </Col>

          <Col>
            <CountValueTable
              title={"Least Alerting Signatures"}
              label={"Signature"}
              rows={leastAlerts()}
            />
          </Col>
        </Row>

        <Row>
          <Col class={"mt-2"}>
            <CountValueTable
              title={"Most Alerting Source Addresses"}
              label={"Address"}
              rows={mostSourceAddrs()}
            />
          </Col>
          <Col class={"mt-2"}>
            <CountValueTable
              title={"Most Alerting Destination Addresses"}
              label={"Address"}
              rows={mostDestAddrs()}
            />
          </Col>
        </Row>
      </Container>
    </>
  );
}

export function CountValueTable(props: {
  title: string;
  label: string;
  rows: { count: number; key: any }[];
}) {
  const showNoData = true;
  return (
    <Show when={showNoData || props.rows.length > 0}>
      <Card>
        <Card.Header>
          <b>{props.title}</b>
        </Card.Header>
        <Show when={props.rows.length === 0}>
          <Card.Body>No results.</Card.Body>
        </Show>
        <Show when={props.rows.length > 0}>
          <Card.Body style={"padding: 0;"}>
            <Table size={"sm"} hover striped>
              <thead>
                <tr>
                  <th>#</th>
                  <th>{props.label}</th>
                </tr>
              </thead>
              <tbody>
                <For each={props.rows}>
                  {(row) => (
                    <tr>
                      <td>{row.count}</td>
                      <td>{row.key}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </Table>
          </Card.Body>
        </Show>
      </Card>
    </Show>
  );
}
