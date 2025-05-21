// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Top } from "./Top";
import * as API from "./api";
import {
  createEffect,
  createMemo,
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
import { useNavigate, useSearchParams } from "@solidjs/router";
import { formatEventDescription } from "./formatters";
import { BiCaretRightFill, BiDashCircle, BiPlusCircle } from "./icons";
import tinykeys from "tinykeys";
import { scrollToClass } from "./scroll";
import { Transition } from "solid-transition-group";
import { eventIsArchived, eventSetArchived } from "./event";
import { AlertDescription } from "./Alerts";
import { EventsQueryParams } from "./api";
import { AddressCell, FilterStrip, TimestampCell } from "./components";

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
  { name: "mDNS", eventType: "mdns" },
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
    from?: string;
    to?: string;
    f?: string[];
  }>();
  const [cursor, setCursor] = createSignal(0);
  const [filters, setFilters] = createSignal<string[]>([]);
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

  // Getter for searchParams.filters to convert to an array if there
  // is only one "filters" parameter.
  const getFilters = createMemo(() => {
    let filters = searchParams.f || [];

    if (!Array.isArray(filters)) {
      return [filters];
    } else {
      return filters;
    }
  });

  // Effect to update the filter strip based on the filters in the query string.
  createEffect(() => {
    setFilters(getFilters());
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
    let params: EventsQueryParams = {
      query_string: searchParams.q || "",
    };

    if (searchParams.event_type) {
      params.event_type = searchParams.event_type;
      setEventType(params.event_type);
    }

    const filterQuery: string = filters()?.join(" ");
    if (filterQuery && filterQuery.length > 0) {
      params.query_string += " " + filterQuery;
    }

    if (searchParams.order) {
      params.order = searchParams.order;
    }

    if (searchParams.from) {
      params.from = searchParams.from;
    }

    if (searchParams.to) {
      params.to = searchParams.to;
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
      from: undefined,
      to: undefined,
    });
  }

  function gotoNewer() {
    // Removing "-" and ":" are for URL tidyness, but don't matter.
    const timestamp = events()[0]
      ._source["@timestamp"].replace(/(\d{4})-(\d{2})-(\d{2})/, "$1$2$3")
      .replaceAll(":", "");
    setSearchParams({
      order: "asc",
      from: timestamp,
      to: undefined,
    });
  }

  function gotoOlder() {
    // Removing "-" and ":" are for URL tidyness, but don't matter.
    const timestamp = events()
      [events().length - 1]._source["@timestamp"].replace(
        /(\d{4})-(\d{2})-(\d{2})/,
        "$1$2$3",
      )
      .replaceAll(":", "");
    setSearchParams({
      order: undefined,
      from: undefined,
      to: timestamp,
    });
  }

  function gotoNewest() {
    setSearchParams({
      order: undefined,
      to: undefined,
      from: undefined,
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

  function addFilter(what: string, op: string, value: any) {
    if (op == "+") {
      op = "";
    }
    let entry: string = "";
    if (typeof value === "number") {
      entry = `${op}${what}:${value}`;
    } else if (value.includes(" ")) {
      entry = `${op}${what}:"${value}"`;
    } else {
      entry = `${op}${what}:${value}`;
    }

    let newFilters = filters();

    // If if entry already exists.
    if (newFilters.indexOf(entry) > -1) {
      return;
    }

    newFilters.push(entry);
    setSearchParams({
      f: newFilters,
    });
  }

  // Effect to update the query parameters when the filters signal updates.
  createEffect(() => {
    setSearchParams({
      f: filters().length == 0 ? undefined : filters(),
    });
  });

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

        <Show when={filters().length != 0}>
          <FilterStrip filters={filters} setFilters={setFilters} />
        </Show>

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
                                    addFilter={addFilter}
                                  />
                                </td>
                                <td class={"col-event-type"}>
                                  {event._source.event_type?.toUpperCase() ||
                                    "???"}
                                  <span
                                    class="show-on-hover ms-1"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      addFilter(
                                        "event_type",
                                        "+",
                                        event._source.event_type,
                                      );
                                    }}
                                    title={`Filter for event_type: ${event._source.event_type}`}
                                  >
                                    <BiPlusCircle />
                                  </span>
                                  <span
                                    class="show-on-hover ms-1"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      addFilter(
                                        "event_type",
                                        "-",
                                        event._source.event_type,
                                      );
                                    }}
                                    title={`Filter out event_type: ${event._source.event_type}`}
                                  >
                                    <BiDashCircle />
                                  </span>
                                </td>
                                <td class={"col-address"} style={"width: 0%;"}>
                                  <Switch fallback={<>{event._source.host}</>}>
                                    <Match when={event._source.src_ip}>
                                      <AddressCell
                                        source={event._source}
                                        fn={addFilter}
                                      />
                                    </Match>
                                    <Match
                                      when={
                                        event._source.arp &&
                                        event._source.arp.src_ip
                                      }
                                    >
                                      <AddressCell
                                        source={event._source.arp}
                                        fn={addFilter}
                                      />
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
