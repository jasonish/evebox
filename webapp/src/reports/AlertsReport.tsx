// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { createEffect, createSignal, For, Show } from "solid-js";
import * as API from "../api";
import { Button, Card, Col, Container, Row, Table } from "solid-bootstrap";
import { GroupByQueryRequest } from "../api";
import { RefreshButton } from "../common/RefreshButton";

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

  createEffect(() => {
    refresh(TIME_RANGE());
  });

  function forceRefresh() {
    console.log("AlertsReport.forceRefresh");
    refresh(TIME_RANGE());
  }

  function refresh(timeRange: string) {
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
