// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import {
  Outlet,
  useLocation,
  useNavigate,
  useSearchParams,
} from "@solidjs/router";
import { TIME_RANGE, Top } from "./Top";
import {
  Badge,
  Button,
  ButtonGroup,
  Col,
  Container,
  Dropdown,
  Form,
  Row,
} from "solid-bootstrap";
import {
  batch,
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
import * as API from "./api";
import { QUEUE_SIZE } from "./api";
import { parse_timerange } from "./datetime";
import { EventWrapper } from "./types";
import { BiCaretRightFill, BiStar, BiStarFill, BiStarHalf } from "./icons";
import tinykeys from "tinykeys";
import { scrollToClass } from "./scroll";
import { Transition } from "solid-transition-group";
import { VIEW_SIZE } from "./settings";
import {
  eventIsArchived,
  eventSetArchived,
  eventSetEscalated,
  Tag,
} from "./event";
import { AddressCell, TimestampCell } from "./TimestampCell";
import { IdleTimer } from "./idletimer";
import { eventStore } from "./eventstore";
import { AppProtoBadge } from "./Events";

enum View {
  Inbox,
  Escalated,
  Alerts,
}

export function AlertState() {
  console.log("***** AlertState *****");

  onMount(() => {
    eventStore.reset();
  });

  return (
    <>
      <Outlet />
    </>
  );
}

export function Alerts() {
  console.log("***** Alerts *****");
  const location = useLocation();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams<{
    q: string;
    offset: string;
  }>();
  const [cursor, setCursor] = createSignal(0);
  const [isLoading, setIsLoading] = createSignal(false);
  const idleTimer = new IdleTimer(60000);
  const [visibleEvents, setVisibleEvents] = createSignal<EventWrapper[]>([]);

  let toggleSelectAllRef: HTMLInputElement | null = null;
  let bindings: any = null;
  let view: View | undefined = undefined;

  switch (location.pathname) {
    case "/inbox":
      view = View.Inbox;
      break;
    case "/escalated":
      view = View.Escalated;
      break;
    case "/alerts":
      view = View.Alerts;
      break;
    default:
      console.log(
        `Location path ${location.pathname} not supported here, redirecting to /inbox`
      );
      navigate("/inbox");
      break;
  }

  onMount(() => {
    console.log("Inbox.onMount");

    bindings = tinykeys(window, {
      j: () => {
        setCursor((c) => (c < visibleEvents().length - 1 ? c + 1 : c));
        scrollToClass("event-row", cursor());
      },
      k: () => {
        setCursor((c) => (c > 0 ? c - 1 : 0));
        scrollToClass("event-row", cursor());
      },
      "Shift+h": () => {
        setCursor(0);
        scrollToClass("event-row", cursor());
      },
      "Shift+g": () => {
        if (visibleEvents().length > 0) {
          setCursor(visibleEvents().length - 1);
          scrollToClass("event-row", cursor());
        }
      },
      ".": () => {
        let element = document.getElementsByClassName("action-toggle")[
          cursor()
        ] as HTMLElement;
        if (element) {
          element.click();
          element.focus();
        }
      },
      e: () => {
        if (visibleEvents().length > 0) {
          if (!archiveSelected()) {
            console.log("No selected rows to archive, will archive at cursor.");
            archive(cursor());
          }
        }
      },
      s: () => {
        if (visibleEvents().length > 0) {
          if (!escalateSelected()) {
            escalate(cursor());
          }
        }
      },
      x: () => {
        toggleSelected(cursor());
      },
      "Shift+x": () => {
        const event = visibleEvents()[cursor()];
        if (event) {
          selectBySignatureId(event._source.alert?.signature_id!);
        }
      },
      r: () => {
        onRefresh();
      },
      f8: () => {
        archive(cursor());
      },
      o: () => {
        openEventAtCursor();
      },
      "/": (e) => {
        const input = document.getElementById("filter-input");
        if (input) {
          input.focus();
        }
        e.preventDefault();
      },
      "Shift+* a": () => {
        console.log("Toggle select all");
        toggleSelectAll();
      },
    });

    if (eventStore.events.length > 0 && view == View.Inbox) {
      // Find events that may now be archived.
      let i = eventStore.events.length;
      while (i--) {
        if (eventIsArchived(eventStore.events[i])) {
          eventStore.events.splice(i, 1);
        }
      }
      if (eventStore.cursor > eventStore.events.length - 1) {
        setCursor(eventStore.events.length - 1);
      } else {
        setCursor(eventStore.cursor);
      }
    }

    if (eventStore.events.length === 0) {
      if (QUEUE_SIZE() === 0) {
        refreshEvents({ query_string: searchParams.q });
      }
    }

    console.log("Alerts.onMount: done");
  });

  onCleanup(() => {
    if (bindings) {
      bindings();
    }
    idleTimer.stop();
  });

  function getOffset(): number {
    return +searchParams.offset || 0;
  }

  function setOffset(offset: number) {
    batch(() => {
      setSearchParams({ offset: offset === 0 ? undefined : offset });
      setCursor(0);
    });
  }

  // Update the visible events as the offset is changed.
  createEffect(() => {
    console.log(`Alerts.createEffect: Updating visible events.`);
    batch(() => {
      setVisibleEvents(
        eventStore.events.slice(getOffset(), getOffset() + VIEW_SIZE())
      );
      if (visibleEvents().length === 0 && getOffset() > 0) {
        console.log(`- No more visible events, moving to previous page.`);
        setOffset(getOffset() - VIEW_SIZE());
      }
    });
  });

  createEffect(() => {
    if (idleTimer.timeout()) {
      console.log("Alerts.createEffect: Idle timeout: refreshing");
      untrack(() => {
        onRefresh();
      });
    }
  });

  // Manage the state of the select all checkbox.
  createEffect(() => {
    let checked = false;
    let indeterminate = false;
    const selected = getSelected();
    if (selected.length === 0) {
      checked = false;
      indeterminate = false;
    } else if (selected.length === visibleEvents().length) {
      checked = true;
      indeterminate = false;
    } else {
      checked = true;
      indeterminate = true;
    }
    if (toggleSelectAllRef) {
      toggleSelectAllRef!.checked = checked;
      toggleSelectAllRef!.indeterminate = indeterminate;
    }
  });

  function toggleSelected(i: number) {
    let event = visibleEvents()[i];
    if (!event.__private) {
      event.__private = {
        selected: false,
      };
    }
    event.__private.selected = !event.__private.selected;
  }

  createEffect(
    (prev: any) => {
      console.log(
        `Alerts.createEffect: query_string: previous=${prev.q}, current=${searchParams.q}`
      );
      console.log(
        `Alerts.createEffect: time_range: previous=${
          prev.time_range
        }, current=${TIME_RANGE()}`
      );

      if (searchParams.q != prev.q || prev.time_range != TIME_RANGE()) {
        console.log(`- Search parameters have changed, refreshing events.`);
        refreshEvents({ query_string: searchParams.q });
      } else {
        console.log(`- No change to search parameters`);
      }
      return { q: searchParams.q || undefined, time_range: TIME_RANGE() };
    },
    { q: searchParams.q || undefined, time_range: TIME_RANGE() }
  );

  function onRefresh() {
    refreshEvents({ query_string: searchParams.q });
  }

  function refreshEvents(options: { query_string?: string } = {}) {
    console.log("Alerts.refreshEvents: " + JSON.stringify(options));
    let params: any = {};

    untrack(() => {
      let time_range = TIME_RANGE();
      if (time_range) {
        params.time_range = parse_timerange(time_range) || undefined;
      }
    });

    if (options.query_string) {
      params.query_string = options.query_string;
    }
    switch (view) {
      case View.Inbox:
        params.tags = [`-${Tag.Archived}`];
        break;
      case View.Escalated:
        params.tags = [`${Tag.Escalated}`];
        break;
      default:
        break;
    }

    setIsLoading(true);

    API.alerts(params)
      .then((response) => {
        const events: EventWrapper[] = response.events;
        events.sort((a, b) => {
          if (a._metadata!.max_timestamp < b._metadata!.max_timestamp) {
            return -1;
          } else if (a._metadata!.max_timestamp > b._metadata!.max_timestamp) {
            return 1;
          } else {
            return 0;
          }
        });
        events.forEach((event) => {
          event.__private = {
            selected: false,
          };
        });
        events.reverse();
        if (eventStore.events.length === 0 && events.length === 0) {
          // Do nothing...
        } else {
          batch(() => {
            eventStore.events = events;
            eventStore.active = null;
          });
        }
      })
      .finally(() => {
        setIsLoading(false);
      });
  }

  const applyFilter = (filter: string) => {
    console.log("applyFilter: " + filter);
    setSearchParams({ q: filter });
  };

  const clearFilter = () => {
    setSearchParams({ q: undefined });
  };

  // Get the indexes of all event rows that are selected.
  function getSelected(): number[] {
    let selected: number[] = [];
    visibleEvents().forEach((e, i) => {
      if (e.__private?.selected) {
        selected.push(i);
      }
    });
    selected.reverse();
    return selected;
  }

  function archiveSelected(): boolean {
    const selected = getSelected();
    if (selected.length === 0) {
      return false;
    }
    for (const i of selected) {
      archive(i);
    }
    return true;
  }

  function archive(i: number) {
    const event = visibleEvents()[i];
    if (!event) {
      return;
    }

    const allEventsIndex = eventStore.events.indexOf(event);
    console.log(
      `Archiving visible event ${i}, index in all events = ${allEventsIndex}`
    );
    let ignore = API.archiveAggregateAlert(event);
    if (view === View.Inbox) {
      eventStore.events.splice(allEventsIndex, 1);
      if (cursor() > 0 && cursor() > i) {
        setCursor(cursor() - 1);
      }
      if (cursor() > visibleEvents().length - 1) {
        setCursor(Math.max(0, cursor() - 1));
      }
    } else {
      eventSetArchived(event);
    }
  }

  function escalate(i: number) {
    let event = visibleEvents()[i];
    if (event._metadata!.count != event._metadata!.escalated_count) {
      let ignore = API.escalateAggregateAlert(event);

      // Optimistically set event as escalated.
      eventSetEscalated(event);
      event._metadata!.escalated_count = event._metadata!.count;
    } else {
      let ignore = API.unescalateAggregateAlert(event);
      event._metadata!.escalated_count = 0;
    }
  }

  function escalateSelected(): boolean {
    const selected = getSelected();
    if (selected.length === 0) {
      return false;
    }
    for (const i of selected) {
      escalate(i);
    }
    return true;
  }

  function toggleSelectAll() {
    if (getSelected().length > 0) {
      unselectAll();
    } else {
      selectAll();
    }
  }

  function isAllSelected(): boolean {
    return (
      visibleEvents().length > 0 &&
      getSelected().length === visibleEvents().length
    );
  }

  function selectAll() {
    visibleEvents().forEach((event) => {
      event.__private.selected = true;
    });
  }

  function unselectAll() {
    visibleEvents().forEach((event) => {
      event.__private.selected = false;
    });
  }

  function navigateToEvent(event: EventWrapper) {
    // Run in batch so no effects are triggered as we navigate away at the end.
    batch(() => {
      console.log(`Navigating to event ${event._id}`);
      eventStore.setActive(event);
      eventStore.viewOffset = getOffset();
      eventStore.cursor = cursor();
      console.log(`EVENT_STORE.active._id=${eventStore.active?._id}`);
      setTimeout(() => {
        navigate(`${location.pathname}/${event._id}`, {
          state: {
            referer: location.pathname,
          },
        });
      }, 0);
    });
  }

  function openEventAtCursor() {
    let event = visibleEvents()[cursor()];
    navigateToEvent(event);
  }

  function blurInputs() {
    const elementIds = ["filter-input"];
    for (const elementId of elementIds) {
      const element = document.getElementById(elementId);
      if (element) {
        element.blur();
      }
    }
  }

  function selectBySignatureId(signatureId: number) {
    for (let event of visibleEvents()) {
      if (event._source.alert?.signature_id === signatureId) {
        event.__private.selected = true;
      }
    }
  }

  return (
    <>
      <Top />
      <Container fluid class={"mt-2 mb-2"}>
        {/* Debug. */}
        <Show when={localStorage.getItem("DEBUG") !== null}>
          <Row class={"mt-2 mb-2"}>
            <Col>
              {JSON.stringify(
                {
                  "eventStore.events.length": eventStore.events.length,
                  "visibleEvents().length": visibleEvents().length,
                  "eventStore.active._id": eventStore.active?._id || null,
                  "cursor()": cursor(),
                  "eventStore.viewOffset": eventStore.viewOffset,
                  "eventStore.cursor": eventStore.cursor,
                },
                null,
                1
              )}
            </Col>
          </Row>
        </Show>

        {/* For mobile detection. */}
        <div style={"height: 0; width: 0"}>
          <span class={"d-none d-sm-block"}></span>
          <span id="small-only" class={"d-block d-sm-none"}></span>
        </div>

        <Row>
          <Col>
            <Show when={!isLoading()}>
              <button
                class={"btn btn-secondary me-2"}
                style="width: 7em;"
                onclick={onRefresh}
              >
                Refresh
              </button>
            </Show>
            <Show when={visibleEvents().length > 0 && !isAllSelected()}>
              <button
                class={"btn btn-secondary me-2"}
                style="width: 7em;"
                onclick={selectAll}
              >
                Select All
              </button>
            </Show>
            <Show when={isAllSelected()}>
              <button
                class={"btn btn-secondary me-2"}
                style="width: 8em;"
                onclick={unselectAll}
              >
                Unselect All
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
            <Show when={getSelected().length > 0}>
              <button
                class={"btn btn-secondary me-2"}
                onclick={archiveSelected}
              >
                Archive
              </button>
            </Show>
            <Show when={getSelected().length > 0}>
              <button
                class={"btn btn-secondary me-2"}
                onclick={escalateSelected}
              >
                Escalate
              </button>
            </Show>
          </Col>
          <Col>
            <Form
              class="input-group"
              onsubmit={(e) => {
                e.preventDefault();
                blurInputs();
                applyFilter(e.currentTarget.filter.value);
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
                    blurInputs();
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
                onclick={clearFilter}
              >
                Clear
              </button>
            </Form>
          </Col>
        </Row>

        <div
          class="mt-2"
          classList={{
            invisible: isLoading() && eventStore.events.length === 0,
          }}
        >
          <PagerRow
            events={eventStore.events}
            offset={getOffset()}
            setOffset={setOffset}
          />
        </div>

        <Transition name={"fade"}>
          {visibleEvents().length > 0 && (
            <div>
              <table
                class={"table table-hover mt-2 event-table"}
                style={"margin-bottom: 0;"}
              >
                <thead>
                  <tr>
                    <th class={"col-cursor"}></th>
                    <th class={"col-check"}>
                      <input
                        ref={toggleSelectAllRef!}
                        type={"checkbox"}
                        class="form-check-input"
                        onchange={(e) => {
                          e.preventDefault();
                          toggleSelectAll();
                        }}
                      />
                    </th>
                    <th class={"col-star"}></th>
                    <th class={"col-count"}>#</th>
                    <th class="col-timestamp">Timestamp</th>
                    <th class="col-address">Src / Dst</th>
                    <th>Signature</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  <For each={visibleEvents()}>
                    {(event, i) => {
                      let severity = event._source.alert?.severity || 3;
                      let alert = event._source.alert!;
                      return (
                        <>
                          <tr
                            classList={{
                              "evebox-row-info": severity > 2,
                              "evebox-row-warning": severity === 2,
                              "evebox-row-danger": severity === 1,
                              "event-row": true,
                            }}
                            onclick={() => {
                              setCursor(i);
                              navigateToEvent(event);
                            }}
                          >
                            <td>
                              <Show when={cursor() === i()}>
                                <BiCaretRightFill />
                              </Show>
                            </td>
                            <td onclick={(e) => e.stopPropagation()}>
                              <input
                                type={"checkbox"}
                                checked={event.__private?.selected}
                                class={"form-check-input"}
                                style={"margin-top: 7px;"}
                                onchange={() => toggleSelected(i())}
                              />
                            </td>
                            <td
                              onclick={(e) => {
                                e.stopPropagation();
                                escalate(i());
                              }}
                            >
                              <Switch fallback={<BiStar />}>
                                <Match
                                  when={
                                    event._metadata!.count > 0 &&
                                    event._metadata!.count ===
                                      event._metadata!.escalated_count
                                  }
                                >
                                  <BiStarFill />
                                </Match>
                                <Match
                                  when={
                                    event._metadata!.escalated_count > 0 &&
                                    event._metadata!.count >
                                      event._metadata!.escalated_count
                                  }
                                >
                                  <BiStarHalf />
                                </Match>
                              </Switch>
                            </td>
                            <td>
                              <div
                                class={"col-count"}
                                style={"margin-top: 3px;"}
                              >
                                {event._metadata!.count}
                              </div>
                            </td>
                            <td class={"col-timestamp"}>
                              <TimestampCell
                                timestamp={event._source.timestamp}
                              />
                            </td>
                            <td class={"col-address col-1"}>
                              <AddressCell source={event._source} />
                            </td>
                            <td>
                              <AlertDescription event={event} />
                            </td>
                            <Show when={eventIsArchived(event)}>
                              <td></td>
                            </Show>
                            <Show when={!eventIsArchived(event)}>
                              <td onclick={(e) => e.stopPropagation()}>
                                <Dropdown
                                  as={ButtonGroup}
                                  class="float-end"
                                  style={"margin-top: 5px !important"}
                                >
                                  <Button
                                    variant="secondary"
                                    onclick={(e) => {
                                      archive(i());
                                    }}
                                  >
                                    Archive
                                  </Button>
                                  <Dropdown.Toggle
                                    split
                                    variant="secondary"
                                    class={"action-toggle"}
                                  />
                                  <Dropdown.Menu>
                                    <Dropdown.Item
                                      onClick={() =>
                                        selectBySignatureId(alert.signature_id)
                                      }
                                    >
                                      Select all with SID {alert.signature_id}
                                    </Dropdown.Item>
                                  </Dropdown.Menu>
                                </Dropdown>
                              </td>
                            </Show>
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

        <Show when={visibleEvents().length > 0}>
          <div
            class="mt-2"
            classList={{
              invisible: isLoading() && eventStore.events.length === 0,
            }}
          >
            <PagerRow
              events={eventStore.events}
              offset={getOffset()}
              setOffset={setOffset}
            />
          </div>
        </Show>
      </Container>
    </>
  );
}

export function AlertDescription(props: { event: EventWrapper }) {
  const alert = props.event._source.alert!;
  return (
    <>
      <Show when={alert.action !== "allowed"}>
        <Badge class={"bg-warning me-1"}>{alert.action.toUpperCase()}</Badge>
      </Show>
      {alert.signature}
      <AppProtoBadge event={props.event} class={"ms-2"} />
    </>
  );
}

function PagerRow(props: {
  events: EventWrapper[];
  offset: number;
  setOffset: any;
}) {
  const [first, setFirst] = createSignal(props.offset + 1);
  const [last, setLast] = createSignal(props.offset + VIEW_SIZE());

  createEffect(() => {
    setFirst(props.offset + 1);
    if (props.offset + 1 + VIEW_SIZE() < props.events.length) {
      setLast(props.offset + VIEW_SIZE());
    } else {
      setLast(props.events.length);
    }
  });

  function gotoOlder() {
    const next = props.offset + VIEW_SIZE();
    if (next < props.events.length) {
      props.setOffset(next);
    }
  }

  function gotoOldest() {
    const pages = Math.floor(props.events.length / VIEW_SIZE());
    props.setOffset(pages * VIEW_SIZE());
  }

  const NoEvents = () => (
    <div class={"row mt-2"}>
      <div class={"col"}>No events found.</div>
    </div>
  );

  return (
    <>
      <Show when={props.events.length > 0} fallback={<NoEvents />}>
        <Row>
          <div class={"col-md-6 col-sm-12 mt-2"}>
            Alerts {first()}-{last()} of {props.events.length}
          </div>
          <div class={"col-md-6 col-sm-12"}>
            <div class="btn-group float-end" role="group">
              <button
                type="button"
                class="btn btn-secondary"
                onclick={() => props.setOffset(0)}
                disabled={props.offset === 0}
              >
                Newest
              </button>
              <button
                type="button"
                class="btn btn-secondary"
                disabled={first() === 1}
                onclick={() => props.setOffset(props.offset - VIEW_SIZE())}
              >
                Newer
              </button>
              <button
                type="button"
                class="btn btn-secondary"
                disabled={props.offset + VIEW_SIZE() > props.events.length}
                onclick={gotoOlder}
              >
                Older
              </button>
              <button
                type="button"
                class="btn btn-secondary"
                disabled={last() == props.events.length}
                onclick={gotoOldest}
              >
                Oldest
              </button>
            </div>
          </div>
        </Row>
      </Show>
    </>
  );
}
