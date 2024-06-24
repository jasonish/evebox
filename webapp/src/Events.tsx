// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "./Top";
import * as API from "./api";
import {
  createEffect,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { EventWrapper } from "./types";
import { Button, Col, Container, Form, Row } from "solid-bootstrap";
import { AddressCell, TimestampCell } from "./TimestampCell";
import { useNavigate, useSearchParams } from "@solidjs/router";
import { formatEventDescription } from "./formatters";
import { BiCaretRightFill } from "./icons";
import tinykeys from "tinykeys";
import { scrollToClass } from "./scroll";
import { Transition } from "solid-transition-group";
import { eventIsArchived, eventSetArchived } from "./event";
import { AlertDescription } from "./Alerts";
import { EventsQueryParams } from "./api";

// The list of event types that will be shown in dropdowns.
export const EVENT_TYPES: { name: string; eventType: string }[] = [
  { name: "All", eventType: "" },
  { name: "Alert", eventType: "alert" },
  { name: "ARP", eventType: "arp" },
  { name: "Anomaly", eventType: "anomaly" },
  { name: "DCERPC", eventType: "dcerpc" },
  { name: "DHCP", eventType: "dhcp" },
  { name: "DNP3", eventType: "dnp3" },
  { name: "DNS", eventType: "dns" },
  { name: "Drop", eventType: "drop" },
  { name: "FileInfo", eventType: "fileinfo" },
  { name: "Flow", eventType: "flow" },
  { name: "Frame", eventType: "frame" },
  { name: "FTP", eventType: "ftp" },
  { name: "FTP Data", eventType: "ftp_data" },
  { name: "HTTP", eventType: "http" },
  { name: "IKE", eventType: "ike" },
  { name: "KRB5", eventType: "krb5" },
  { name: "Modbus", eventType: "modbus" },
  { name: "MQTT", eventType: "mqtt" },
  { name: "NetFlow", eventType: "netflow" },
  { name: "NFS", eventType: "nfs" },
  { name: "PostgreSQL", eventType: "pgsql" },
  { name: "QUIC", eventType: "quic" },
  { name: "RDP", eventType: "rdp" },
  { name: "RFB", eventType: "rfb" },
  { name: "SIP", eventType: "sip" },
  { name: "SMB", eventType: "smb" },
  { name: "SMTP", eventType: "smtp" },
  { name: "SNMP", eventType: "snmp" },
  { name: "SSH", eventType: "ssh" },
  { name: "Stats", eventType: "stats" },
  { name: "TFTP", eventType: "tftp" },
  { name: "TLS", eventType: "tls" },
];

export function eventNameFromType(type: string): string | undefined {
  for (const et of EVENT_TYPES) {
    if (et.eventType === type) {
      return et.name;
    }
  }
  return undefined;
}

export function Events() {
  const navigate = useNavigate();
  const [isLoading, setIsLoading] = createSignal(false);
  const [events, setEvents] = createSignal<EventWrapper[]>([], {
    equals: (prev, next) => {
      return false;
    },
  });
  const [eventType, setEventType] = createSignal(EVENT_TYPES[0].eventType);
  const [searchParams, setSearchParams] = useSearchParams<{
    q?: string;
    order?: "asc";
    event_type?: string;
    min_timestamp?: string;
    max_timestamp?: string;
  }>();
  const [cursor, setCursor] = createSignal(0);
  let keybindings: any = null;

  onMount(() => {
    console.log("Events.onMount");
    keybindings = tinykeys(window, {
      j: () => {
        setCursor((c) => (c < events().length - 1 ? c + 1 : c));
        scrollToClass("event-row", cursor());
      },
      k: () => {
        setCursor((c) => (c > 0 ? c - 1 : 0));
        scrollToClass("event-row", cursor());
      },
      o: () => {
        const event = events()[cursor()];
        if (event) {
          openEventById(event._id);
        }
      },
      "/": (e) => {
        document.getElementById("searchInput")!.focus();
        e.preventDefault();
      },
      r: () => {
        setEvents([]);
        loadEvents();
      },
      e: () => {
        archive(cursor());
      },
      f8: () => {
        archive(cursor());
      },
    });
  });

  onCleanup(() => {
    if (keybindings) {
      keybindings();
    }
  });

  createEffect(() => {
    loadEvents();
  });

  function openEventById(id: string) {
    navigate(`/event/${id}`, {
      state: {
        referer: location.pathname,
      },
    });
  }

  function loadEvents() {
    let params: EventsQueryParams = {};

    if (searchParams.event_type) {
      params.event_type = searchParams.event_type;
      setEventType(params.event_type);
    }

    if (searchParams.q) {
      params.query_string = searchParams.q;
    }

    if (searchParams.order) {
      params.order = searchParams.order;
    }

    if (searchParams.min_timestamp) {
      params.min_timestamp = searchParams.min_timestamp;
    }

    if (searchParams.max_timestamp) {
      params.max_timestamp = searchParams.max_timestamp;
    }

    setIsLoading(true);

    API.getEvents(params)
      .then((response) => {
        if (response.events) {
          console.log(`Received ${response.events.length} events`);
          const events: EventWrapper[] = response.events;
          if (searchParams.order === "asc") {
            events.reverse();
          }

          setEvents(events);
        } else {
          console.log(`ERROR: Response contained no data`);
          console.log(response);
          setEvents([]);
        }
      })
      .finally(() => {
        setIsLoading(false);

        if (events().length > 0 && cursor() > events().length - 1) {
          setCursor(events().length - 1);
        } else if (events().length === 0) {
          setCursor(0);
        }
      });
  }

  function gotoOldest() {
    setSearchParams({
      order: "asc",
      min_timestamp: undefined,
      max_timestamp: undefined,
    });
  }

  function gotoNewer() {
    const timestamp = events()[0]._source["@timestamp"];
    setSearchParams({
      order: "asc",
      min_timestamp: timestamp,
      max_timestamp: undefined,
    });
  }

  function gotoOlder() {
    const timestamp = events()[events().length - 1]._source["@timestamp"];
    setSearchParams({
      order: undefined,
      min_timestamp: undefined,
      max_timestamp: timestamp,
    });
  }

  function gotoNewest() {
    setSearchParams({
      order: undefined,
      max_timestamp: undefined,
      min_timestamp: undefined,
    });
  }

  function archive(i: number) {
    let event = events()[i];
    if (event._source.event_type !== "alert") {
      return;
    }
    eventSetArchived(event);
    API.archiveEvent(event).then(() => {});
    setEvents((events) => {
      events[i] = { ...event };
      return events;
    });
  }

  return (
    <>
      <Top disableRange />
      <Container fluid>
        <Row>
          <div class={"col-auto mt-2"}>
            <Show when={!isLoading()}>
              <button
                class={"btn btn-secondary me-2"}
                style="width: 7em;"
                onclick={loadEvents}
              >
                Refresh
              </button>
            </Show>
            <Show when={isLoading()}>
              <button
                class={"btn btn-secondary me-2"}
                style={"width: 7em;"}
                disabled
              >
                Loading
              </button>
            </Show>
          </div>

          <div class="col-auto mt-2">
            <div class={"row align-items-center"}>
              <label for={"event-type-selector"} class={"col-auto"}>
                Event Type:
              </label>
              <div class={"col-auto"}>
                <select
                  class="form-select"
                  id={"event-type-selector"}
                  onchange={(e) => {
                    setSearchParams({ event_type: e.currentTarget.value });
                    e.currentTarget.blur();
                  }}
                >
                  <For each={EVENT_TYPES}>
                    {(et) => {
                      return (
                        <>
                          <option
                            value={et.eventType}
                            selected={et.eventType === eventType()}
                          >
                            {et.name}
                          </option>
                        </>
                      );
                    }}
                  </For>
                </select>
              </div>
            </div>
          </div>

          <Col class={"mt-2"}>
            <Form
              class="input-group"
              onsubmit={(e) => {
                e.preventDefault();
                setSearchParams({ q: e.currentTarget.searchInput.value });
                const inputs = e.currentTarget.getElementsByTagName("input");
                for (let input of inputs) {
                  input.blur();
                }
              }}
            >
              <input
                id="searchInput"
                type="text"
                class="form-control"
                name="searchInput"
                placeholder="Search..."
                value={searchParams.q || ""}
                onkeydown={(e) => {
                  if (
                    e.code === "Escape" ||
                    e.key === "Escape" ||
                    e.keyCode === 27
                  ) {
                    e.currentTarget.blur();
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
          <div class={"col-md-6 col-sm-12 mt-2 me-auto"}></div>
          <div class={"col-md-6 col-sm-12 mt-2"}>
            <div class={"float-end"}>
              <button
                type="button"
                class="btn btn-secondary ms-2"
                onclick={gotoNewest}
                disabled={isLoading()}
              >
                Newest
              </button>
              <button
                type="button"
                class="btn btn-secondary ms-2"
                onclick={gotoNewer}
                disabled={isLoading()}
              >
                Newer
              </button>
              <button
                type="button"
                class="btn btn-secondary ms-2"
                onclick={gotoOlder}
                disabled={isLoading()}
              >
                Older
              </button>
              <button
                type="button"
                class="btn btn-secondary ms-2"
                onclick={gotoOldest}
                disabled={isLoading()}
              >
                Oldest
              </button>
            </div>
          </div>
        </Row>

        <Row>
          <Col class={"mt-2"}>
            <Transition name={"fade"}>
              {events().length > 0 && (
                <div>
                  <table class={"table table-sm table-hover app-event-table"}>
                    <thead>
                      <tr>
                        <th class={"app-w-1"}></th>
                        <th class={"col-timestamp"}>Timestamp</th>
                        <th class={"col-event-type"}>Type</th>
                        <th class={"col-address"}>Src/Dst</th>
                        <th>Description</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={events()}>
                        {(event, i) => {
                          let severity = event._source.alert?.severity;
                          return (
                            <>
                              <tr
                                onclick={() => openEventById(event._id)}
                                classList={{
                                  "table-info": severity! > 2,
                                  "table-warning": severity === 2,
                                  "table-danger": severity === 1,
                                  "table-success": severity === undefined,
                                }}
                              >
                                <td
                                  class={"app-w-1"}
                                  style={"min-width: 1.5em !important;"}
                                >
                                  <Show when={cursor() === i()}>
                                    <BiCaretRightFill />
                                  </Show>
                                </td>
                                <td class={"col-timestamp"}>
                                  <TimestampCell
                                    timestamp={event._source.timestamp}
                                  />
                                </td>
                                <td class={"col-event-type"}>
                                  {event._source.event_type.toUpperCase()}
                                </td>
                                <td class={"col-address"} style={"width: 0%;"}>
                                  <Switch fallback={<>{event._source.host}</>}>
                                    <Match when={event._source.src_ip}>
                                      <AddressCell source={event._source} />
                                    </Match>
                                    <Match
                                      when={
                                        event._source.arp &&
                                        event._source.arp.src_ip
                                      }
                                    >
                                      <AddressCell source={event._source.arp} />
                                    </Match>
                                  </Switch>
                                </td>
                                <td class={"force-wrap col-auto"}>
                                  <Switch
                                    fallback={
                                      <>
                                        {formatEventDescription(event)}
                                        <AppProtoBadge
                                          event={event}
                                          class={"ms-2"}
                                        />
                                      </>
                                    }
                                  >
                                    <Match
                                      when={
                                        event._source.event_type === "alert"
                                      }
                                    >
                                      <AlertDescription event={event} />
                                    </Match>
                                  </Switch>

                                  <Show when={showArchiveButton(event)}>
                                    <Button
                                      variant="secondary"
                                      class="float-end"
                                      style={"margin-top: 4px;"}
                                      onclick={(e) => {
                                        e.stopPropagation();
                                        archive(i());
                                      }}
                                    >
                                      Archive
                                    </Button>
                                  </Show>
                                </td>
                              </tr>
                            </>
                          );
                        }}
                      </For>
                    </tbody>
                  </table>
                </div>
              )}
            </Transition>
          </Col>
        </Row>
      </Container>
    </>
  );
}

function showArchiveButton(event: EventWrapper) {
  return event._source.event_type === "alert" && !eventIsArchived(event);
}

export function AppProtoBadge(props: {
  event: EventWrapper;

  class?: string;
}) {
  if (
    !props.event._source.app_proto ||
    props.event._source.app_proto === "failed"
  ) {
    return <></>;
  }
  return (
    <span class={`badge bg-secondary ${props.class}`}>
      {props.event._source.app_proto}
    </span>
  );
}
