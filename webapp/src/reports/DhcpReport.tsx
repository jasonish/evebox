// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Card, Col, Container, Row, Table } from "solid-bootstrap";
import { TIME_RANGE, Top } from "../Top";
import * as api from "../api";
import { createEffect, createSignal, For } from "solid-js";
import { EventSource } from "../types";
import { parse_timestamp } from "../datetime";
import { useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { RefreshButton } from "../common/RefreshButton";
import { loadingTracker } from "../util";
import { SearchLink } from "../common/SearchLink";

export function DhcpReport() {
  const [acks, setAcks] = createSignal<EventSource[]>([]);
  const [dhcpServers, setDhcpServers] = createSignal<string[]>([]);
  const [searchParams, setSearchParams] = useSearchParams();
  const [loading, setLoading] = createSignal(0);

  createEffect(() => {
    refresh();
  });

  function refresh() {
    loadingTracker(setLoading, async () => {
      const query = { time_range: TIME_RANGE(), sensor: searchParams.sensor };

      const response = await api.dhcpRequest(query);
      let requestHostnames: { [key: number]: string } = {};
      for (const event of response.events) {
        if (event.dhcp?.hostname) {
          requestHostnames[event.dhcp.id] = event.dhcp?.hostname;
        }
      }
      const response_1 = await api.dhcpAck(query);
      response_1.events.forEach((event_1: EventSource) => {
        const hostname = requestHostnames[event_1.dhcp!.id];
        if (hostname) {
          if (!event_1.dhcp!.hostname) {
            event_1.dhcp!.hostname = hostname;
          } else if (event_1.dhcp!.hostname != hostname) {
            event_1.dhcp!.hostname = `${event_1.dhcp?.hostname} (${hostname})`;
          }
        }
      });
      setAcks(response_1.events);
    });

    let sensor = "";
    if (searchParams.sensor) {
      sensor = ` host:${searchParams.sensor}`;
    }

    loadingTracker(setLoading, async () => {
      const response = await api.fetchAgg({
        field: "src_ip",
        size: 100,
        time_range: TIME_RANGE(),
        order: "desc",
        q: `event_type:dhcp dhcp.dhcp_type:ack${sensor}`,
      });
      let servers = response.rows.map((e) => e.key);
      setDhcpServers(servers);
    });
  }

  return (
    <>
      <Top />
      <Container fluid={true}>
        <Row>
          <Col class={"pt-2 col-auto"}>
            <RefreshButton loading={loading()} refresh={refresh} />
          </Col>
          <Col class={"pt-2 col-auto"}>
            <SensorSelect
              onchange={(sensor) => {
                setSearchParams({ sensor: sensor });
              }}
              selected={searchParams.sensor}
            />
          </Col>
        </Row>
        <Row>
          <Col class={"mt-2"} md={9}>
            <Card>
              <Card.Header>DHCP Leases</Card.Header>
              <Card.Body class={"p-0"}>
                <Table size={"sm"} hover={true} striped={true} class="mb-0">
                  <thead>
                    <tr>
                      <th class={"ps-2"}>Timestamp</th>
                      <th>Sensor</th>
                      <th>Client MAC</th>
                      <th>Assigned IP</th>
                      <th>Hostname</th>
                      <th>Lease Time</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={acks()}>
                      {(ack) => (
                        <>
                          <tr>
                            <td class={"ps-2"}>
                              {parse_timestamp(ack.timestamp).format(
                                "YYYY-MM-DD HH:mm:ss"
                              )}
                            </td>
                            <td>{ack.host}</td>
                            <td>
                              <SearchLink
                                value={ack.dhcp!.client_mac}
                                field="@mac"
                              >
                                {ack.dhcp!.client_mac}
                              </SearchLink>
                            </td>
                            <td>
                              <SearchLink
                                value={ack.dhcp!.assigned_ip}
                                field="@ip"
                              >
                                {ack.dhcp!.assigned_ip}
                              </SearchLink>
                            </td>
                            <td>
                              <SearchLink value={ack.dhcp!.hostname}>
                                {ack.dhcp!.hostname}
                              </SearchLink>
                            </td>
                            <td>{ack.dhcp!.lease_time}</td>
                          </tr>
                        </>
                      )}
                    </For>
                  </tbody>
                </Table>
              </Card.Body>
            </Card>
          </Col>

          <Col md={3}>
            <Row>
              <Col class={"pt-2"} sm={6} md={12}>
                <Card>
                  <Card.Header>DHCP Servers</Card.Header>
                  <Card.Body class={"p-0"}>
                    <Table
                      size={"sm"}
                      class={"mb-0"}
                      hover={true}
                      striped={true}
                    >
                      <tbody class={""}>
                        <For each={dhcpServers()}>
                          {(server) => (
                            <>
                              <tr>
                                <td class={"ps-2"}>
                                  <SearchLink value={server} field="@ip">
                                    {server}
                                  </SearchLink>
                                </td>
                              </tr>
                            </>
                          )}
                        </For>
                      </tbody>
                    </Table>
                  </Card.Body>
                </Card>
              </Col>
            </Row>
          </Col>
        </Row>
      </Container>
    </>
  );
}
