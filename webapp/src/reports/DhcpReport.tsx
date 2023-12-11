// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import {
  Alert,
  Button,
  Card,
  Col,
  Container,
  Form,
  Row,
  Table,
} from "solid-bootstrap";
import { TIME_RANGE, Top } from "../Top";
import { API } from "../api";
import { createEffect, createSignal, For, Setter, Show } from "solid-js";
import { EventSource } from "../types";
import { parse_timestamp } from "../datetime";
import { A, useSearchParams } from "@solidjs/router";
import { SensorSelect } from "../common/SensorSelect";
import { RefreshButton } from "../common/RefreshButton";
import { trackLoading } from "../util";
import { RawSearchLink, SearchLink } from "../common/SearchLink";

export function DhcpReport() {
  const [acks, setAcks] = createSignal<EventSource[]>([]);
  const [dhcpServers, setDhcpServers] = createSignal<string[]>([]);
  const [dhcpClients, setDhcpClients] = createSignal<string[]>([]);
  const [searchParams, setSearchParams] = useSearchParams();
  const [loading, setLoading] = createSignal(0);

  createEffect(() => {
    refresh();
  });

  function refresh() {
    trackLoading(setLoading, () => {
      const query = { time_range: TIME_RANGE(), sensor: searchParams.sensor };

      return API.dhcpRequest(query).then((response) => {
        let requestHostnames: { [key: number]: string } = {};
        for (const event of response.events) {
          if (event.dhcp?.hostname) {
            requestHostnames[event.dhcp.id] = event.dhcp?.hostname;
          }
        }
        return API.dhcpAck(query).then((response) => {
          response.events.forEach((event: EventSource) => {
            const hostname = requestHostnames[event.dhcp!.id];
            if (hostname) {
              if (!event.dhcp!.hostname) {
                event.dhcp!.hostname = hostname;
              } else if (event.dhcp!.hostname != hostname) {
                event.dhcp!.hostname = `${event.dhcp?.hostname} (${hostname})`;
              }
            }
          });
          setAcks(response.events);
        });
      });
    });

    let sensor = "";
    if (searchParams.sensor) {
      sensor = ` host:${searchParams.sensor}`;
    }

    trackLoading(setLoading, () => {
      return API.groupBy({
        field: "dhcp.client_mac",
        size: 100,
        time_range: TIME_RANGE(),
        order: "desc",
        q: `event_type:dhcp dhcp.dhcp_type:request${sensor}`,
      }).then((response) => {
        let clients = response.rows.map((e) => e.key);
        setDhcpClients(clients);
      });
    });

    trackLoading(setLoading, () => {
      return API.groupBy({
        field: "src_ip",
        size: 100,
        time_range: TIME_RANGE(),
        order: "desc",
        q: `event_type:dhcp dhcp.dhcp_type:ack${sensor}`,
      }).then((response) => {
        let servers = response.rows.map((e) => e.key);
        setDhcpServers(servers);
      });
    });
  }

  function earliest(timestamp: string): string {
    return parse_timestamp(timestamp)
      .subtract(1, "minute")
      .format("YYYY-MM-DDTHH:mm:ss.sssZZ");
  }

  function latest(timestamp: string): string {
    return parse_timestamp(timestamp)
      .add(1, "minute")
      .format("YYYY-MM-DDTHH:mm:ss.sssZZ");
  }

  return (
    <>
      <Top />
      <Container fluid={true}>
        <Row>
          <Col class={"pt-2 col-auto"}>
            <RefreshButton
              loading={loading()}
              refresh={refresh}
              showProgress={true}
            />
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
                              <RawSearchLink
                                q={`event_type:dhcp dhcp.id:${
                                  ack.dhcp?.id
                                } @earliest:"${earliest(
                                  ack.timestamp
                                )}" @latest:"${latest(ack.timestamp)}"`}
                              >
                                {parse_timestamp(ack.timestamp).format(
                                  "YYYY-MM-DD HH:mm:ss"
                                )}
                              </RawSearchLink>
                            </td>
                            <td>{ack.host}</td>
                            <td>
                              <SearchLink value={ack.dhcp!.client_mac}>
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
              <Col class={"pt-2"} sm={6} md={12}>
                <Show
                  when={dhcpClients().length > 0}
                  fallback={
                    <>
                      <Alert>No DHCP clients</Alert>
                    </>
                  }
                >
                  <Card>
                    <Card.Header>DHCP Clients</Card.Header>
                    <Card.Body class={"p-0"}>
                      <Table
                        size={"sm"}
                        class={"mb-0"}
                        hover={true}
                        striped={true}
                      >
                        <tbody class={""}>
                          <For each={dhcpClients()}>
                            {(addr) => (
                              <>
                                <tr>
                                  <td class={"ps-2"}>
                                    <SearchLink value={addr}>{addr}</SearchLink>
                                  </td>
                                </tr>
                              </>
                            )}
                          </For>
                        </tbody>
                      </Table>
                    </Card.Body>
                  </Card>
                </Show>
              </Col>
            </Row>
          </Col>
        </Row>
      </Container>
    </>
  );
}
