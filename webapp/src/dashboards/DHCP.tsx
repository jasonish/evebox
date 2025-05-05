// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Removing solid-bootstrap imports
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

export function DHCP() {
  const [acks, setAcks] = createSignal<EventSource[]>([]);
  const [dhcpServers, setDhcpServers] = createSignal<string[]>([]);
  const [searchParams, setSearchParams] = useSearchParams<{
    sensor?: string;
  }>();
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
        if (!event_1["dhcp"]) {
          console.log(
            `DHCP ACK entry does not contain DHCP object: ${JSON.stringify(
              event_1,
            )}`,
          );
        } else {
          const hostname = requestHostnames[event_1.dhcp!.id];
          if (hostname) {
            if (!event_1.dhcp!.hostname) {
              event_1.dhcp!.hostname = hostname;
            } else if (event_1.dhcp!.hostname != hostname) {
              event_1.dhcp!.hostname = `${event_1.dhcp?.hostname} (${hostname})`;
            }
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
      <div class="container-fluid">
        <div class="row">
          <div class="pt-2 col-auto">
            <RefreshButton loading={loading()} refresh={refresh} />
          </div>
          <div class="pt-2 col-auto">
            <SensorSelect
              onchange={(sensor) => {
                setSearchParams({ sensor: sensor });
              }}
              selected={searchParams.sensor}
            />
          </div>
        </div>
        <div class="row">
          <div class="mt-2 col-md-9">
            <div class="card">
              <div class="card-header">DHCP Leases</div>
              <div class="card-body p-0">
                <table class="table table-sm table-hover table-striped mb-0">
                  <thead>
                    <tr>
                      <th class="ps-2">Timestamp</th>
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
                            <td class="ps-2">
                              {parse_timestamp(ack.timestamp).format(
                                "YYYY-MM-DD HH:mm:ss",
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
                </table>
              </div>
            </div>
          </div>

          <div class="col-md-3">
            <div class="row">
              <div class="pt-2 col-sm-6 col-md-12">
                <div class="card">
                  <div class="card-header">DHCP Servers</div>
                  <div class="card-body p-0">
                    <table class="table table-sm mb-0 table-hover table-striped">
                      <tbody>
                        <For each={dhcpServers()}>
                          {(server) => (
                            <>
                              <tr>
                                <td class="ps-2">
                                  <SearchLink value={server} field="@ip">
                                    {server}
                                  </SearchLink>
                                </td>
                              </tr>
                            </>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
