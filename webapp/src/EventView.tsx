// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { A, useLocation, useNavigate, useParams } from "@solidjs/router";
import { Top } from "./Top";
import {
  createEffect,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
  untrack,
} from "solid-js";
import { API, getEventById } from "./api";
import { archiveAggregateAlert } from "./api";
import {
  Button,
  Card,
  Col,
  Container,
  Row,
  Tab,
  Tabs,
  Toast,
} from "solid-bootstrap";
import { prettyPrintJson } from "pretty-print-json";
import { AggregateAlert, EcsGeo, EveDns, Event, EventWrapper } from "./types";
import { parse_timestamp } from "./datetime";
import { formatAddressWithPort, formatEventDescription } from "./formatters";
import tinykeys from "tinykeys";
import {
  eventIsArchived,
  eventIsEscalated,
  eventSetArchived,
  eventSetEscalated,
  eventUnsetEscalated,
} from "./event";
import { eventStore } from "./eventstore";
import { addNotification } from "./Notifications";
import { eventNameFromType } from "./Events";
import { EventServiceConfig, serverConfig } from "./config";
import { createStore } from "solid-js/store";
import { BiInfoCircle } from "./icons";
import { SearchLink } from "./common/SearchLink";

const PCAP_BUTTON_STYLE =
  "--bs-btn-padding-y: .1rem; --bs-btn-padding-x: .2rem; --bs-btn-font-size: .7rem;";

export function EventView() {
  console.log("***** EventView *****");
  const params = useParams<{ id: string }>();
  const [eventId, setEventId] = createSignal<null | string>(null);
  const [event, setEvent] = createSignal<EventWrapper>();
  const [eventDetails, setEventDetails] = createSignal<any[][]>();
  const [commonDetails, setCommonDetails] = createSignal<any[][]>();
  const [showCopyToast, setShowCopyToast] = createSignal(false);
  const [geoIp, setGeoIp] = createStore<{
    source: EcsGeo | undefined;
    destination: EcsGeo | undefined;
  }>({ source: undefined, destination: undefined });
  const navigate = useNavigate();
  const location = useLocation();
  let keybindings: any = null;
  let referer: null | string = null;
  let rawJsonRef: any = null;

  console.log(`- EventView: EVENT_STORE.active_id=${eventStore.active?._id}`);

  onMount(() => {
    keybindings = tinykeys(window, {
      u: () => {
        window.history.back();
      },
      e: () => {
        archiveAlert();
      },
      f8: () => {
        archiveAlert();
      },
    });

    referer = (location.state as any)?.referer || null;
    if (referer) {
      console.log("EventView.onMount: referer=" + referer);
    }
  });

  onCleanup(() => {
    if (keybindings) {
      keybindings();
    }
  });

  createEffect(() => {
    setEventId(params.id);
  });

  // Update GeoIP information.
  createEffect(() => {
    // Check for SELKS style first.
    if (event()?._source.geoip) {
      const geoip = event()?._source.geoip;
      if (geoip?.country_name) {
        let geo = {
          country_name: geoip?.country_name,
        };
        let source = undefined;
        let destination = undefined;
        if (geoip.ip === event()?._source.src_ip) {
          source = geo;
        } else {
          destination = geo;
        }
        setGeoIp({ source: source, destination: destination });
        return;
      }
    }

    const source =
      event()?._source.source?.geo || event()?._source.geoip_source;
    const destination =
      event()?._source.destination?.geo || event()?._source.geoip_destination;

    // Elastic ECS or EveBox log reader.
    setGeoIp({
      source: source && Object.keys(source).length ? source : undefined,
      destination:
        destination && Object.keys(destination).length
          ? destination
          : undefined,
    });
  });

  createEffect(() => {
    console.log(`EventView.createEffect: Loading event ID: ${eventId()}`);

    untrack(() => {
      console.log(`-- Requested event ID: ${params.id}`);
      console.log(`-- Active event ID: ${eventStore.active?._id}`);
      console.log(`-- Events in store: ${eventStore.events.length}`);

      if (eventStore.active && eventStore.active._id == params.id) {
        console.log(
          `EventView.createEffect: Requested event ID is active in EVENT_STORE`
        );
        setEvent(eventStore.active);
      } else {
        console.log("Event.createEffect: Fetching event by ID: " + params.id);
        getEventById(params.id)
          .then((event) => {
            setEvent(event);
          })
          .catch(() => {
            setEvent(undefined);
          });
      }
    });
  });

  createEffect(() => {
    let source = event()?._source;

    if (!source) {
      setEventDetails(undefined);
      setCommonDetails(undefined);
      return;
    }

    function SearchLink(props: { children?: any; field?: string; value: any }) {
      let q;
      switch (typeof props.value) {
        case "number":
          q = encodeURIComponent(
            `${props.field ? props.field + ":" : ""}${props.value}`
          );
          break;
        default:
          q = encodeURIComponent(
            `${props.field ? props.field + ":" : ""}"${props.value}"`
          );
          break;
      }
      return <A href={`/events?q=${q}`}>{props.children || props.value}</A>;
    }

    let commonDetails = [];

    if (source.timestamp) {
      commonDetails.push([
        "Timestamp",
        <SearchLink field={"timestamp"} value={source.timestamp}>
          {formatTimestamp(source.timestamp)}
        </SearchLink>,
      ]);
    }
    if (source.host) {
      commonDetails.push(["Sensor", source.host]);
    }
    if (source.proto) {
      commonDetails.push(["Protocol", source.proto]);
    }
    if (source.src_ip) {
      commonDetails.push([
        "Source",
        <>
          <SearchLink field={"@ip"} value={source.src_ip}>
            {formatAddressWithPort(source.src_ip, source.src_port)}
          </SearchLink>
          <A class={"ps-1"} href={`/reports/address/${source.src_ip}`}>
            <BiInfoCircle class={"bi-inline"} />
          </A>
        </>,
      ]);
    }
    if (source.dest_ip) {
      commonDetails.push([
        "Destination",
        <>
          <SearchLink field={"@ip"} value={source.dest_ip}>
            {formatAddressWithPort(source.dest_ip, source.dest_port)}
          </SearchLink>
          <A class={"ps-1"} href={`/reports/address/${source.dest_ip}`}>
            <BiInfoCircle class={"bi-inline"} />
          </A>
        </>,
      ]);
    }
    if (source.in_iface) {
      commonDetails.push(["In Interface", source.in_iface]);
    }
    if (source.flow_id) {
      commonDetails.push([
        "Flow ID",
        <SearchLink field={"flow_id"} value={source.flow_id} />,
      ]);
    }
    if (source.community_id) {
      commonDetails.push([
        "Community ID",
        <SearchLink field={"community_id"} value={source.community_id} />,
      ]);
    }
    setCommonDetails(commonDetails);

    if (event()?._source.event_type === "alert") {
      let alert = event()!._source.alert!;
      let eventDetails = [
        [
          "Signature",
          <SearchLink field={"alert.signature"} value={alert.signature} />,
        ],
        [
          "Category",
          <SearchLink field={"alert.category"} value={alert.category} />,
        ],
        ["Severity", alert.severity],
        [
          "Signature ID",
          <SearchLink
            field={"alert.signature_id"}
            value={alert.signature_id}
          />,
        ],
        ["Generator ID", alert.gid],
        ["Revision", alert.rev],
      ];
      setEventDetails(eventDetails);
    } else {
      setEventDetails(undefined);
    }
  });

  function copyRawJson() {
    const e: HTMLInputElement | null = document.getElementById(
      "raw-json"
    ) as HTMLInputElement;
    if (e) {
      e.select();
      window.navigator.clipboard.writeText(e.value);
      addNotification("JSON copied to clipboard");
    }
  }

  // From: https://stackoverflow.com/a/2838358
  //
  // Copied from Stack Overflow.  Highlights the text inside an element.
  function selectElementText(el: HTMLElement) {
    if (window.getSelection && document.createRange) {
      const sel = window.getSelection();
      if (sel) {
        const range = document.createRange();
        range.selectNodeContents(el);
        sel.removeAllRanges();
        sel.addRange(range);
        return;
      }
    }
    addNotification("Select not supported in this browser");
  }

  function selectRawJson() {
    const e: HTMLInputElement | null = document.getElementById(
      "formatted-json"
    ) as HTMLInputElement;
    if (e) {
      selectElementText(e);
    }
  }

  function isAggregateAlert(): boolean {
    return event()?._metadata?.count! > 0;
  }

  function isEscalated(): boolean {
    let ev = event();
    if (ev) {
      if (ev._metadata) {
        if (
          ev._metadata.escalated_count > 0 &&
          ev._metadata.escalated_count === ev._metadata.count
        ) {
          return true;
        }
      } else {
        return eventIsEscalated(ev);
      }
    }
    return false;
  }

  function archiveAlert() {
    if (event()?._metadata) {
      const alert = event() as AggregateAlert;
      archiveAggregateAlert(alert).then(() => {});
      eventSetArchived(alert);
    }

    goBack();
  }

  function escalate() {
    let ev = event();
    if (ev) {
      if (isAggregateAlert()) {
        void API.escalateAggregateAlert(ev);
        ev._metadata!.escalated_count = ev._metadata!.count;
      } else {
        void API.escalateEvent(ev);
      }
      eventSetEscalated(ev);
      setEvent({ ...ev });
    }
  }

  function deEscalate() {
    let ev = event();
    if (ev) {
      if (isAggregateAlert()) {
        void API.deEscalateAggregateAlert(ev);
        ev._metadata!.escalated_count = 0;
      } else {
        void API.deEscalateEvent(ev);
      }
      eventUnsetEscalated(ev);
      setEvent({ ...ev });
    }
  }

  // Go "back". If it appears like we came here from an internal click on event, use the back button
  // so state is restored.  If not, go back "parent" view.
  function goBack() {
    if (location.state) {
      window.history.back();
    } else if (location.pathname.startsWith("/escalated")) {
      navigate("/escalated");
    } else if (location.pathname.startsWith("/inbox")) {
      navigate("/inbox");
    } else if (location.pathname.startsWith("/alerts")) {
      navigate("/alerts");
    } else if (location.pathname.startsWith("/event")) {
      navigate("/events");
    }
  }

  function OccurrenceLink(props: { children: any }) {
    if (event()?._source.alert) {
      const alert = event()!._source.alert!;
      let parts = [
        `alert.signature_id:${alert.signature_id}`,
        `src_ip:${event()?._source.src_ip}`,
        `dest_ip:${event()?._source.dest_ip}`,
        `@latest:"${event()?._metadata?.max_timestamp}"`,
        `@earliest:"${event()?._metadata?.min_timestamp}"`,
      ];
      const url = "/events?q=" + parts.map(encodeURIComponent).join("+");
      return <A href={url}>{props.children}</A>;
    } else {
      return <></>;
    }
  }

  interface DisplayObject {
    title: string;
    key: string;
    rows: { key: string; val: any }[];
  }

  const [displayObjects, setDisplayObjects] = createSignal<DisplayObject[]>([]);

  const [objectColumns, setObjectColumns] = createStore<{
    col1: DisplayObject[];
    col2: DisplayObject[];
  }>({
    col1: [],
    col2: [],
  });

  createEffect(() => {
    let objects: DisplayObject[] = [];
    if (event()) {
      let source = event()!._source;
      for (const key of Object.keys(event()!._source)) {
        if (typeof source[key] === "object") {
          const flattened = flattenJson(source[key]);
          if (flattened.length > 0) {
            objects.push({
              key: key,
              title: eventNameFromType(key) || key.toUpperCase(),
              rows: flattened,
            });
          }
        }
      }
      objects.sort((a, b) => {
        if (a.rows.length < b.rows.length) {
          return 1;
        } else if (a.rows.length > b.rows.length) {
          return -1;
        } else {
          return 0;
        }
      });
      setDisplayObjects(objects);

      let card1_len = 0;
      let card2_len = 0;
      let card1: DisplayObject[] = [];
      let card2: DisplayObject[] = [];

      objects.forEach((object) => {
        if (card1_len === 0 || card1_len < card2_len) {
          card1.push(object);
          card1_len += object.rows.length;
          for (const row of object.rows) {
            if (row.val.length > 80) {
              card1_len += Math.ceil(row.val.length / 80);
            }
          }
        } else {
          card2.push(object);
          card2_len += object.rows.length;
          for (const row of object.rows) {
            if (row.val.length > 80) {
              card2_len += Math.ceil(row.val.length / 80);
            }
          }
        }
      });

      setObjectColumns({ col1: card1, col2: card2 });
    }
  });

  function getServiceLinks(event: EventWrapper): any[] {
    let serviceLinks = [];
    const eventServices = serverConfig?.[
      "event-services"
    ] as EventServiceConfig[];
    if (eventServices) {
      for (let service of eventServices) {
        if (!service.enabled) {
          continue;
        }
        let url = service.url.replace(
          "{{raw}}",
          encodeURIComponent(JSON.stringify(event._source))
        );
        if (serviceLinks.length > 0) {
          serviceLinks.push(" | ");
        }
        serviceLinks.push(<A href={url}>{service.name}</A>);
      }
    }

    return serviceLinks;
  }

  function eventToPcap(what: "packet" | "payload") {
    if (event()) {
      API.eventToPcap(event()!, what);
    }
  }

  return (
    <>
      <Top />
      <Container fluid={true} class={"mb-2"}>
        <Row>
          <Col class={"mt-2"}>
            <Button variant={"secondary"} class={"me-2"} onclick={goBack}>
              Back
            </Button>
            <Show when={event() && event()?._source.event_type === "alert"}>
              <Show when={eventIsArchived(event()!)}>
                <Button
                  variant={"outline-secondary"}
                  disabled={true}
                  class="me-2"
                >
                  Archive
                </Button>
              </Show>
              <Show when={!eventIsArchived(event()!)}>
                <Button
                  variant={"secondary"}
                  onclick={archiveAlert}
                  class={"me-2"}
                >
                  Archive{" "}
                  <Show when={isAggregateAlert()}>
                    ({event()?._metadata?.count})
                  </Show>
                </Button>
              </Show>
              <Show when={!isEscalated()}>
                <Button variant={"secondary"} onclick={escalate}>
                  Escalate
                </Button>
              </Show>
              <Show when={isEscalated()}>
                <Button variant={"secondary"} onclick={deEscalate}>
                  De-escalate
                </Button>
              </Show>
            </Show>
          </Col>
        </Row>

        <Show when={event()}>
          <div
            class={`mt-2 mb-2 alert ${bgClassForSeverity(event()!)}`}
            style={"padding: 0.5em;"}
          >
            <div class={"row"}>
              <div class={"col col-auto me-auto fw-bold"}>
                {formatTitle(event()!)}
              </div>
              <div class={"col col-auto"}>
                <Show when={getServiceLinks(event()!).length > 0}>
                  [ {getServiceLinks(event()!)} ]
                </Show>
                <Show when={isAggregateAlert()}>
                  &nbsp; [{" "}
                  <OccurrenceLink>
                    {event()?._metadata!.count} Occurrences
                  </OccurrenceLink>{" "}
                  ]
                </Show>
              </div>
            </div>
          </div>

          <Row>
            <Col class={"mb-2"} lg={12} xl={6}>
              <Card>
                <Card.Body class={"p-0"}>
                  <table
                    class={
                      "table table-sm table-borderless table-striped table-hover app-detail-table"
                    }
                  >
                    <tbody>
                      <For each={commonDetails()}>
                        {(e) => (
                          <>
                            <tr>
                              <td>
                                <b>{e[0]}</b>
                              </td>
                              <td>{e[1]}</td>
                            </tr>
                          </>
                        )}
                      </For>
                    </tbody>
                  </table>
                </Card.Body>
              </Card>
            </Col>
            <Show when={eventDetails()}>
              <Col class={"mb-2"} lg={12} xl={6}>
                <Card>
                  <Card.Body class={"p-0"}>
                    <table
                      class={
                        "table table-sm app-detail-table table-borderless table-striped table-hover"
                      }
                    >
                      <tbody>
                        <For each={eventDetails()!}>
                          {(e) => (
                            <>
                              <tr>
                                <td style={"min-width: 8em;"}>
                                  <b>{e[0]}</b>
                                </td>
                                <td>{e[1]}</td>
                              </tr>
                            </>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </Card.Body>
                </Card>
              </Col>
            </Show>

            <Show when={event()?._source.event_type === "dns"}>
              <Col class={"mb-2"} lg={12} xl={6}>
                <DnsInfoCol dns={event()?._source.dns!} />
              </Col>
            </Show>
          </Row>

          {/* Rule */}
          <Show when={event()?._source?.alert?.rule}>
            <Row class={"mb-2"}>
              <Col>
                <Card>
                  <Card.Header>Rule</Card.Header>
                  <Card.Body>
                    <HighlightedRule rule={event()?._source.alert?.rule!} />
                  </Card.Body>
                </Card>
              </Col>
            </Row>
          </Show>

          {/* GeoIP */}
          <Show when={geoIp.source || geoIp.destination}>
            <Row class={"mb-2"}>
              <Col>
                <Card>
                  <Card.Header>GeoIP</Card.Header>
                  <Card.Body class={"p-0"}>
                    <table
                      class={
                        "mb-0 table table-sm table-striped table-bordered app-detail-table"
                      }
                    >
                      <tbody>
                        <Show when={geoIp.source}>
                          <tr>
                            <td>
                              <b>Source</b>
                            </td>
                            <td>{geoIpInfoString(geoIp.source)}</td>
                          </tr>
                        </Show>
                        <Show when={geoIp.destination}>
                          <tr>
                            <td>
                              <b>Destination</b>
                            </td>
                            <td>{geoIpInfoString(geoIp.destination)}</td>
                          </tr>
                        </Show>
                      </tbody>
                    </table>
                  </Card.Body>
                </Card>
              </Col>
            </Row>
          </Show>

          {/* Tabbed? */}
          <Row>
            <Col class={"mb-2"}>
              <Card class={""} style={""}>
                <Card.Body class={"p-0"}>
                  <Tabs defaultActiveKey={"All"}>
                    <Tab eventKey="All" title="All">
                      {/* Object Cards */}
                      <Row>
                        <For each={[objectColumns.col1, objectColumns.col2]}>
                          {(col) => (
                            <>
                              <Col class={"col-sm-12 col-md-6"}>
                                <For each={col}>
                                  {(o) => (
                                    <>
                                      <Card class={"m-2 event-detail-card"}>
                                        <Card.Header>{o.title}</Card.Header>
                                        <Card.Body class={"p-0"}>
                                          <table
                                            class={
                                              "mb-0 table table-sm table-striped table-bordered app-detail-table"
                                            }
                                          >
                                            <tbody>
                                              <For each={o.rows}>
                                                {(e) => (
                                                  <>
                                                    <tr>
                                                      <td>{e.key}</td>
                                                      <td class="force-wrap">
                                                        <Switch
                                                          fallback={
                                                            <SearchLink
                                                              value={e.val}
                                                            >
                                                              {e.val}
                                                            </SearchLink>
                                                          }
                                                        >
                                                          <Match
                                                            when={
                                                              e.val === true ||
                                                              e.val === false
                                                            }
                                                          >
                                                            {`${e.val}`}
                                                          </Match>
                                                          <Match
                                                            when={
                                                              o.key ==
                                                                "alert" &&
                                                              e.key == "rule"
                                                            }
                                                          >
                                                            <SearchLink
                                                              field={
                                                                "alert.signature"
                                                              }
                                                              value={
                                                                event()?._source
                                                                  .alert
                                                                  ?.signature
                                                              }
                                                            >
                                                              {e.val}
                                                            </SearchLink>
                                                          </Match>
                                                        </Switch>
                                                      </td>
                                                    </tr>
                                                  </>
                                                )}
                                              </For>
                                            </tbody>
                                          </table>
                                        </Card.Body>
                                      </Card>
                                    </>
                                  )}
                                </For>
                              </Col>
                            </>
                          )}
                        </For>
                      </Row>
                    </Tab>
                    <For each={displayObjects()}>
                      {(t, i) => {
                        return (
                          <>
                            <Tab eventKey={t.key} title={t.title}>
                              <table
                                class={
                                  "mb-0 table table-sm table-striped table-bordered app-detail-table"
                                }
                              >
                                <tbody>
                                  <For each={t.rows}>
                                    {(e) => (
                                      <>
                                        <tr>
                                          <th class={""} style={"width: 1%;"}>
                                            {e.key}
                                          </th>
                                          <td class="force-wrap">
                                            <Switch
                                              fallback={
                                                <SearchLink value={e.val}>
                                                  {e.val}
                                                </SearchLink>
                                              }
                                            >
                                              <Match
                                                when={
                                                  t.key == "alert" &&
                                                  e.key == "rule"
                                                }
                                              >
                                                <SearchLink
                                                  field={"alert.signature"}
                                                  value={
                                                    event()?._source.alert
                                                      ?.signature
                                                  }
                                                >
                                                  {e.val}
                                                </SearchLink>
                                              </Match>
                                            </Switch>
                                          </td>
                                        </tr>
                                      </>
                                    )}
                                  </For>
                                </tbody>
                              </table>
                            </Tab>
                          </>
                        );
                      }}
                    </For>
                  </Tabs>
                </Card.Body>
              </Card>
            </Col>
          </Row>

          {/* Payload */}
          <Show when={event()?._source.payload}>
            <Row class={"mb-2"}>
              <Col>
                <Base64BufferCard
                  title={"Payload"}
                  buffer={event()!._source.payload}
                  addOn={
                    <Button
                      onclick={() => eventToPcap("payload")}
                      style={PCAP_BUTTON_STYLE}
                    >
                      PCAP
                    </Button>
                  }
                />
              </Col>
            </Row>
          </Show>

          {/* Packet */}
          <Show when={event()?._source.packet}>
            <Row class={"mb-2"}>
              <Col>
                <Base64BufferCard
                  title={"Packet"}
                  buffer={event()!._source.packet}
                  addOn={
                    <Button
                      onclick={() => eventToPcap("packet")}
                      style={PCAP_BUTTON_STYLE}
                    >
                      PCAP
                    </Button>
                  }
                />
              </Col>
            </Row>
          </Show>

          <Row>
            <Col class={"mt-2"} sm={12} xxl={6}>
              <Card>
                <Card.Header>Event Listing</Card.Header>
                <Card.Body class="p-0">
                  <table
                    class={
                      "mb-0 table table-sm table-striped table-bordered app-detail-table"
                    }
                  >
                    <tbody>
                      <For each={flattenJson(event())}>
                        {(e) => (
                          <>
                            <tr>
                              <td>{e.key}</td>
                              <td class="force-wrap">{e.val}</td>
                            </tr>
                          </>
                        )}
                      </For>
                    </tbody>
                  </table>
                </Card.Body>
              </Card>
            </Col>
            <Col class={"mt-2"} sm={12} xxl={6}>
              <Card>
                <Card.Header>
                  JSON
                  <div class={"small float-end"}>
                    [
                    <Show
                      when={
                        window.location.protocol === "https:" ||
                        window.location.hostname === "localhost" ||
                        window.location.hostname === "127.0.0.1"
                      }
                    >
                      <a
                        href={""}
                        onclick={(e) => {
                          e.preventDefault();
                          copyRawJson();
                        }}
                      >
                        Copy
                      </a>
                      |
                    </Show>
                    <a
                      href={""}
                      onclick={(e) => {
                        e.preventDefault();
                        selectRawJson();
                      }}
                    >
                      Select
                    </a>
                    ]
                  </div>
                  <div style={"position: relative"}>
                    <div class={""} style={"position: fixed; right: 22px;"}>
                      <Toast
                        onClose={() => setShowCopyToast(false)}
                        show={showCopyToast()}
                        autohide
                        delay={10000}
                      >
                        <Toast.Body>JSON copied to clipboard</Toast.Body>
                      </Toast>
                    </div>
                  </div>
                </Card.Header>
                <Card.Body>
                  <PrettyJson id={"formatted-json"} json={event()} />
                </Card.Body>
              </Card>
            </Col>
          </Row>

          <textarea ref={rawJsonRef} id={"raw-json"} style={"display: none;"}>
            {JSON.stringify(event(), undefined, 4)}
          </textarea>
        </Show>
      </Container>
    </>
  );
}

function toPrettyHex(data: string): [string, string][] {
  let output: [string, string][] = [];
  let chars = [];

  for (let i = 0; i < data.length; i++) {
    chars.push(data.charCodeAt(i));
  }

  while (chars.length > 0) {
    const chunk = chars.splice(0, 16);
    let hex = [];
    let printable = [];
    for (let i = 0; i < chunk.length; i++) {
      const x = chunk[i].toString(16);
      if (x.length === 1) {
        hex.push("0" + x);
      } else {
        hex.push(x);
      }
      if (chunk[i] >= 32 && chunk[i] <= 127) {
        printable.push(String.fromCharCode(chunk[i]));
      } else {
        printable.push(".");
      }
    }
    output.push([hex.join(" "), printable.join("")]);
  }

  return output;
}

function formatTitle(event: Event): string {
  try {
    return `${event._source.event_type.toUpperCase()}: ${formatEventDescription(
      event
    )}`;
  } catch (err) {
    return JSON.stringify(event);
  }
}

function bgClassForSeverity(event: Event) {
  switch (event._source.alert?.severity) {
    case 1:
      return "alert-danger";
    case 2:
      return "alert-warning";
    default:
      return "alert-info";
  }
}

function formatTimestamp(timestamp: string) {
  const ts = parse_timestamp(timestamp);
  return ts.format("YYYY-MM-DD HH:mm:ss.SSS");
}

function flattenJson(
  object: any,
  prefix: string[] = [],
  output: { key: string; val: any }[] = []
): { key: string; val: any }[] {
  if (object === null) {
    return output;
  }
  for (const key of Object.keys(object)) {
    let key_prefix = [...prefix];
    key_prefix.push(key);
    let val = object[key];
    switch (typeof val) {
      case "object":
        flattenJson(val, key_prefix, output);
        break;
      default:
        const full_key = key_prefix.join(".");
        if (!full_key.startsWith("__private.")) {
          output.push({ key: key_prefix.join("."), val: val });
        }
        break;
    }
  }

  return output;
}

function PrettyJson(props: any) {
  let output: any = undefined;

  // Copy and delete client side private fields.
  const json = { ...props.json };
  delete json.__private;

  createEffect(() => {
    if (output) {
      output.innerHTML = prettyPrintJson.toHtml(json);
    }
  });

  return (
    <>
      <pre
        ref={output}
        class="json-container force-wrap"
        id={"formatted-json"}
        style={props.style}
      ></pre>
    </>
  );
}

function Base64BufferCard(props: {
  title: string;
  buffer: string;
  addOn?: any;
}) {
  return (
    <Card>
      <Card.Header>
        {props.title}
        <Show when={props.addOn}>
          <span class={"float-end"}>{props.addOn}</span>
        </Show>
      </Card.Header>
      <Card.Body class={"p-2"}>
        <Row>
          <Col md={12} xl={6} class={"pb-2"}>
            <Card>
              <Card.Body class={"p-2"}>
                <pre class={"force-wrap"}>{atob(props.buffer)}</pre>
              </Card.Body>
            </Card>
          </Col>
          <Col md={12} xl={6}>
            <Card>
              <Card.Body class={"p-2"}>
                <table class={"m-0 table table-borderless table-striped"}>
                  <tbody>
                    <For each={toPrettyHex(atob(props.buffer))}>
                      {(e) => (
                        <>
                          <tr style={"padding: 0; margin: 0;"}>
                            <td style={"padding: 0; margin: 0;"}>
                              <pre style={"margin: 0; padding: 0;"}>{e[0]}</pre>
                            </td>
                            <td style={"padding: 0; margin: 0;"}>
                              <pre style={"margin: 0; padding: 0;"}>{e[1]}</pre>
                            </td>
                          </tr>
                        </>
                      )}
                    </For>
                  </tbody>
                </table>
              </Card.Body>
            </Card>
          </Col>
        </Row>
      </Card.Body>
    </Card>
  );
}

function HighlightedRule(props: { rule: string }) {
  const [rule, setRule] = createSignal("");

  createEffect(() => {
    let html = props.rule;

    html = html.replace(
      /^([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+/,
      `<span class="rule-header-action">$1</span>
                 <span class="rule-header-proto">$2</span>
                 <span class="rule-header-addr">$3</span>
                 <span class="rule-header-port">$4</span> 
                 <span class="rule-header-direction">$5</span> 
                 <span class="rule-header-addr">$6</span>
                 <span class="rule-header-port">$7</span> `
    );

    html = html.replace(
      /:([^;]+)/g,
      `:<span class="rule-keyword-value">$1</span>`
    );
    html = html.replace(/(\w+\:)/g, `<span class="rule-keyword">$1</span>`);

    // Catch keywords without a value.
    html = html.replace(
      /(;\s*)(\w+;)/g,
      `$1<span class="rule-keyword">$2</span>`
    );

    // Replace referece URLs with the URL.
    html = html.replace(/url,(.*?)([;<])/g, `url,<a href="http://$1">$1</a>$2`);

    html = html.replace("&___lt___", "&lt;");
    html = html.replace("&___gt___", "&gt;");

    setRule(html);
  });

  return <div innerHTML={rule()} class={"rule"}></div>;
}

function DnsInfoCol(props: { dns: EveDns }) {
  interface DataCard {
    title: string | null;
    data: DataCardRow[];
  }

  interface DataCardRow {
    key: string;
    val: any;
  }

  const cards: DataCard[] = [];

  let common = [
    { key: "Type", val: props.dns.type.toUpperCase() },
    { key: "Query", val: `${props.dns.rrtype} ${props.dns.rrname}` },
  ];
  if (props.dns.rcode) {
    common.push({ key: "RCODE", val: props.dns.rcode });
  }

  cards.push({
    title: null,
    data: common,
  });

  if (props.dns.answers) {
    const rows = props.dns.answers.map((a) => {
      return {
        key: `${a.rrtype} ${a.rrname}`,
        val: a.rdata,
      };
    });
    cards.push({
      title: "DNS Answers",
      data: rows,
    });
  }

  if (props.dns.authorities) {
    let rows: DataCardRow[] = [];
    props.dns.authorities.forEach((a) => {
      if (a.rrtype === "SOA" && a.soa) {
        rows.push({
          key: `${a.rrtype} ${a.rrname}`,
          val: `${a.soa?.mname} (${a.soa.rname})`,
        });
      }
    });
    if (rows.length > 0) {
      cards.push({
        title: "DNS Authorities",
        data: rows,
      });
    }
  }

  return (
    <>
      <For each={cards}>
        {(card, i) => (
          <>
            <Card class={"mb-2"}>
              <table class={"table table-striped table-hover"}>
                <Show when={card.title}>
                  <thead>
                    <tr>
                      <th colspan={"2"}>{card.title}</th>
                    </tr>
                  </thead>
                </Show>
                <tbody>
                  <For each={card.data}>
                    {(row) => (
                      <tr>
                        <th>{row.key}</th>
                        <td>{row.val}</td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </Card>
          </>
        )}
      </For>
    </>
  );
}

function geoIpInfoString(geo: any) {
  let parts = [];
  if (geo.continent_name) {
    parts.push(geo.continent_name);
  }
  if (geo.country_name) {
    parts.push(geo.country_name);
  }
  if (geo.region_name) {
    parts.push(geo.region_name);
  }
  if (geo.city_name) {
    parts.push(geo.city_name);
  }
  return parts.join(" / ");
}
