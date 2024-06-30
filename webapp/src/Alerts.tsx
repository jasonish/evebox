// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { useLocation, useNavigate, useSearchParams } from "@solidjs/router";
import { _SET_TIME_RANGE, SET_TIME_RANGE, TIME_RANGE, Top } from "./Top";
import {
  Badge,
  Button,
  ButtonGroup,
  Col,
  Container,
  Dropdown,
  Form,
  OverlayTrigger,
  Row,
  Tooltip,
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
import { parse_timerange, parse_timestamp } from "./datetime";
import { EventWrapper } from "./types";
import {
  BiArchive,
  BiCaretDownFill,
  BiCaretRightFill,
  BiCaretUpFill,
  BiStar,
  BiStarFill,
  BiStarHalf,
} from "./icons";
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
import { Logger } from "./util";
import { SensorSelect } from "./common/SensorSelect";
import * as bootstrap from "bootstrap";

const DEFAULT_SORTBY = "timestamp";
const DEFAULT_SORTORDER = "desc";

enum View {
  Inbox,
  Escalated,
  Alerts,
}

export function AlertState(props: any) {
  console.log("***** AlertState *****");

  onMount(() => {
    eventStore.reset();
  });

  return <>{props.children}</>;
}

export function Alerts() {
  const logger = new Logger("Alerts", true);
  const location = useLocation();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams<{
    q: string;
    offset: string;
    sortBy?: string;
    sortOrder?: "asc" | "desc";
    sensor?: undefined | string;
  }>();
  const [cursor, setCursor] = createSignal(0);
  const [isLoading, setIsLoading] = createSignal(false);
  const idleTimer = new IdleTimer(60000);
  const [visibleEvents, setVisibleEvents] = createSignal<EventWrapper[]>([]);
  const [timedOut, setTimedOut] = createSignal(false);

  let toggleSelectAllRef: HTMLInputElement | null = null;
  let bindings: any = null;
  let view: View | undefined = undefined;

  // If this is the escalated view we'll move to the "All" time range. The time
  // range upon entering the escalated view is stored here and will be returned
  // to when we leave the escalated view.
  let savedTimeRange: undefined | string = undefined;

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
    const logger = new Logger("Alerts.onMount");
    logger.log("Start");

    if (view == View.Escalated) {
      untrack(() => {
        savedTimeRange = TIME_RANGE();
        _SET_TIME_RANGE("");
      });
    }

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
      "Shift+s": () => {
        escalateArchive(cursor());
      },
      x: () => {
        toggleSelected(cursor());
      },
      r: () => {
        refresh();
      },
      f8: () => {
        archive(cursor());
      },
      f9: () => {
        escalateArchive(cursor());
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
        selectAll();
      },
      "Shift+* n": () => {
        unselectAll();
      },
      "Shift+* 1": () => {
        const event = visibleEvents()[cursor()];
        if (event) {
          selectBySignatureId(event._source.alert?.signature_id!);
        }
      },
    });

    if (eventStore.events.length > 0 && view == View.Inbox) {
      // Find events that may now be archived.
      let i = eventStore.events.length;
      while (i--) {
        if (eventIsArchived(eventStore.events[i])) {
          logger.log(`Removing event at index ${i} as it is now archived`);
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
        refresh();
      }
    }

    logger.log("End");
  });

  onCleanup(() => {
    if (bindings) {
      bindings();
    }
    idleTimer.stop();
    untrack(() => {
      if (savedTimeRange) {
        console.log("Restoring time range of " + savedTimeRange);
        SET_TIME_RANGE(savedTimeRange);
      }
    });
  });

  function getOffset(): number {
    return +(searchParams.offset || 0);
  }

  function setOffset(offset: number) {
    batch(() => {
      setSearchParams({ offset: offset === 0 ? undefined : offset });
      setCursor(0);
    });
  }

  // Update the visible events as the offset is changed.
  createEffect(() => {
    const logger = new Logger("Alerts.createEffect: visible events", true);
    batch(() => {
      setVisibleEvents(
        eventStore.events.slice(getOffset(), getOffset() + VIEW_SIZE())
      );
      if (visibleEvents().length === 0 && getOffset() > 0) {
        logger.log("No more visible events, moving to previous page");
        setOffset(getOffset() - VIEW_SIZE());
      }
    });
  });

  createEffect(() => {
    if (idleTimer.timeout()) {
      logger.log("Idle timeout, refreshing");
      refresh();
    }
  });

  // Manage the state of the select all checkbox.
  createEffect(() => {
    let checked = false;
    let indeterminate = false;
    const selected = getSelectedIndexes();
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

  // Effect to subscribe to all actions that should trigger a refresh.
  createEffect((prev) => {
    let _options = {
      // sortBy: searchParams.sortBy,
      // sortOrder: searchParams.sortOrder,
      q: searchParams.q,
      timeRange: TIME_RANGE(),
      sensor: searchParams.sensor,
    };
    if (prev === undefined) {
      logger.log("Initial check of sortBy and sortOrder, not refreshing");
    } else {
      logger.log("Calling onRefresh as sortBy or sortOrder have changed");
      refresh();
    }
    return true;
  });

  createEffect((prev) => {
    let sortBy = searchParams.sortBy || DEFAULT_SORTBY;
    let sortOrder = searchParams.sortOrder || DEFAULT_SORTORDER;
    if (prev) {
      console.log("updating sort order");
      let events: EventWrapper[] = [];
      untrack(() => {
        events.push(...eventStore.events);
      });
      sortAlerts(events, sortBy, sortOrder);
      eventStore.events = events;
    } else {
      console.log("**** IGNORING sort");
    }
    return true;
  });

  function refresh() {
    // Run untracked. Other effects will watch for the required changes and
    // call as needed.  This is to avoid being called on first load unless
    // needed.
    untrack(() => {
      const logger = new Logger("Alerts.refreshEvents", true);
      let params: any = {
        query_string: searchParams.q,
        time_range: parse_timerange(TIME_RANGE()) || undefined,
      };

      if (searchParams.sensor) {
        params.sensor = searchParams.sensor;
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
          setTimedOut(response.timed_out);
          const events: EventWrapper[] = response.events;
          sortAlerts(events, getSortBy(), getSortOrder());
          events.forEach((event) => {
            event.__private = {
              selected: false,
            };
          });

          if (eventStore.events.length === 0 && events.length === 0) {
            // Do nothing...
          } else {
            batch(() => {
              eventStore.events = events;
              eventStore.active = null;
            });
            logger.log(`Fetch ${events.length} events`);
          }
        })
        .finally(() => {
          setIsLoading(false);
        });
    });
  }

  function sortAlerts(
    alerts: EventWrapper[],
    sortBy: string,
    order: string
  ): void {
    console.log(`sortAlerts: sortBy=${sortBy}, order=${order}`);

    function compare(a: any, b: any): number {
      if (a < b) {
        return -1;
      } else if (a > b) {
        return 1;
      }
      return 0;
    }

    switch (sortBy) {
      case "signature":
        alerts.sort((a: any, b: any) => {
          return compare(
            a._source.alert.signature.toUpperCase(),
            b._source.alert.signature.toUpperCase()
          );
        });
        break;
      case "count":
        alerts.sort((a: any, b: any) => {
          return a._metadata.count - b._metadata.count;
        });
        break;
      case "source":
        alerts.sort((a: any, b: any) => {
          return compare(a._source.src_ip, b._source.src_ip);
        });
        break;
      case "dest":
        alerts.sort((a: any, b: any) => {
          return compare(a._source.dest_ip, b._source.dest_ip);
        });
        break;
      case "timestamp":
        alerts.sort((a: any, b: any) => {
          const da = parse_timestamp(a._metadata.max_timestamp);
          const db = parse_timestamp(b._metadata.max_timestamp);
          return compare(da, db);
        });
        break;
    }

    if (order === "desc") {
      console.log(`sortAlerts: reversing as order is descending`);
      alerts.reverse();
    }
  }

  const applyFilter = (filter: string) => {
    console.log("applyFilter: " + filter);
    setSearchParams({ q: filter });
  };

  const clearFilter = () => {
    setSearchParams({ q: undefined });
  };

  // Get the indexes of all event rows that are selected.
  function getSelectedIndexes(): number[] {
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
    const selected = getSelectedIndexes();
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

  function escalate(i: number): Promise<any> {
    let event = visibleEvents()[i];
    if (event._metadata!.count != event._metadata!.escalated_count) {
      // Optimistically set event as escalated.
      eventSetEscalated(event);
      event._metadata!.escalated_count = event._metadata!.count;

      return API.escalateAggregateAlert(event);
    } else {
      event._metadata!.escalated_count = 0;
      return API.unescalateAggregateAlert(event);
    }
  }

  function escalateSelected(): boolean {
    const selected = getSelectedIndexes();
    if (selected.length === 0) {
      return false;
    }
    for (const i of selected) {
      escalate(i);
    }
    return true;
  }

  function toggleSelectAll() {
    if (getSelectedIndexes().length > 0) {
      unselectAll();
    } else {
      selectAll();
    }
  }

  function isAllSelected(): boolean {
    return (
      visibleEvents().length > 0 &&
      getSelectedIndexes().length === visibleEvents().length
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
      navigate(`${location.pathname}/${event._id}`, {
        state: {
          referer: location.pathname,
        },
      });
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

  function filterBySignatureId(signatureId: number) {
    let q = searchParams.q;
    if (q) {
      q = `alert.signature_id:${signatureId} ${q}`;
    } else {
      q = `alert.signature_id:${signatureId}`;
    }
    setSearchParams({ q: q });
  }

  function updateSort(key: string) {
    console.log("Sorting by " + key);
    let order = getSortOrder();
    if (key === getSortBy()) {
      if (order === "asc") {
        order = "desc";
      } else {
        order = "asc";
      }
    }
    setSearchParams({ sortBy: key, sortOrder: order });
  }

  function getSortOrder() {
    return searchParams.sortOrder || DEFAULT_SORTORDER;
  }

  function getSortBy() {
    return searchParams.sortBy || DEFAULT_SORTBY;
  }

  function escalateArchive(index: number) {
    escalate(index).then(() => archive(index));
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
          <Col class="d-flex">
            <Show when={!isLoading()}>
              <button
                class={"btn btn-secondary me-2"}
                style="width: 7em;"
                onclick={refresh}
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
            <Show when={getSelectedIndexes().length > 0}>
              <button
                class={"btn btn-secondary me-2"}
                onclick={archiveSelected}
              >
                Archive
              </button>
            </Show>
            <Show when={getSelectedIndexes().length > 0}>
              <button
                class={"btn btn-secondary me-2"}
                onclick={escalateSelected}
              >
                Escalate
              </button>
            </Show>
            <div class="d-inline-flex">
              <SensorSelect
                selected={"asdf"}
                onchange={(sensor) => {
                  setSearchParams({ sensor: sensor });
                }}
              />
            </div>
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
            timedOut={timedOut()}
          />
        </div>

        <Transition name={"fade"}>
          {visibleEvents().length > 0 && (
            <div>
              <table
                class={"table table-hover mt-2 app-event-table"}
                style={"margin-bottom: 0;"}
              >
                <thead>
                  <tr>
                    <th class={"app-w-1"}></th>
                    <th class={"app-w-1"}>
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
                    <th class={"app-w-1"}></th>
                    <SortHeader
                      title={"#"}
                      key={"count"}
                      sortBy={getSortBy()}
                      sortOrder={getSortOrder()}
                      class={"col-count"}
                      onclick={updateSort}
                    />
                    <SortHeader
                      title={"Timestamp"}
                      key={"timestamp"}
                      sortBy={getSortBy()}
                      sortOrder={getSortOrder()}
                      class={"col-timestamp"}
                      onclick={updateSort}
                    />
                    <th class="col-address" style={"cursor: pointer"}>
                      <span onclick={() => updateSort("source")}>
                        Src{" "}
                        <Show when={getSortBy() === "source"}>
                          <SortCaret order={getSortOrder()}></SortCaret>
                        </Show>
                      </span>
                      /{" "}
                      <span onclick={() => updateSort("dest")}>
                        Dst
                        <Show when={getSortBy() === "dest"}>
                          <SortCaret order={getSortOrder()}></SortCaret>
                        </Show>
                      </span>
                    </th>
                    <SortHeader
                      title={"Signature"}
                      key={"signature"}
                      sortBy={getSortBy()}
                      sortOrder={getSortOrder()}
                      onclick={updateSort}
                    />
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
                              "evebox-row-info table-info": severity > 2,
                              "evebox-row-warning table-warning":
                                severity === 2,
                              "table-danger": severity === 1,
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

                                  <OverlayTrigger
                                    placement="left"
                                    delay={{ show: 700, hide: 100 }}
                                    overlay={
                                      <Tooltip id="button-tooltip">
                                        Escalate and Archive
                                      </Tooltip>
                                    }
                                  >
                                    <Button
                                      variant="secondary"
                                      onclick={() => escalateArchive(i())}
                                      style={"position: relative;"}
                                    >
                                      <BiArchive />
                                      <BiStar
                                        style={
                                          "position: absolute; top: 7px; left: 18px;"
                                        }
                                      />
                                    </Button>
                                  </OverlayTrigger>
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
                                    <Dropdown.Item
                                      onClick={() =>
                                        filterBySignatureId(alert.signature_id)
                                      }
                                    >
                                      Filter on SID {alert.signature_id}
                                    </Dropdown.Item>
                                    <Dropdown.Item
                                      onClick={() => escalateArchive(i())}
                                    >
                                      Escalate and Archive
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
      {alert.signature}{" "}
      <Show when={props.event._source.app_proto != "failed"}>
        <span class="badge text-bg-secondary me-2">
          {props.event._source.app_proto}
        </span>
      </Show>
      <Show when={props.event._source.tls?.sni}>
        <span class="badge text-bg-secondary me-2">
          {props.event._source.tls!.sni}
        </span>
      </Show>
      <Show when={props.event._source.quic?.sni}>
        <span class="badge text-bg-secondary me-2">
          {props.event._source.quic!.sni}
        </span>
      </Show>
      <Show when={props.event._source.dns?.query}>
        <span class="badge text-bg-secondary me-2">
          {props.event._source.dns?.query![0].rrname}
        </span>
      </Show>
      <Show when={props.event._source.http?.hostname}>
        <span class="badge text-bg-secondary me-2">
          {props.event._source.http?.hostname}
        </span>
      </Show>
      <Show
        when={
          props.event._source.tags &&
          (props.event._source.tags.indexOf("evebox.auto-archived") > -1 ||
            props.event._source.tags.indexOf("evebox.auto_archived") > -1)
        }
      >
        <span class="badge text-bg-secondary me-2">auto-archived</span>
      </Show>
    </>
  );
}

function PagerRow(props: {
  events: EventWrapper[];
  offset: number;
  setOffset: any;
  timedOut: boolean;
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

  createEffect(() => {
    const tooltipTriggerList = document.querySelectorAll(
      '[data-bs-toggle="tooltip"]'
    );
    const tooltipList = [...tooltipTriggerList].map(
      (tooltipTriggerEl) => new bootstrap.Tooltip(tooltipTriggerEl)
    );
  });

  return (
    <>
      <Show when={props.events.length > 0} fallback={<NoEvents />}>
        <Row>
          <div class={"col-md-6 col-sm-12 mt-2"}>
            Alerts {first()}-{last()} of {props.events.length}
            <Show when={props.timedOut}>
              {" "}
              <span
                class="badge text-bg-warning align-middle"
                data-bs-toggle="tooltip"
                data-bs-title="Query timed out, not all events will be shown. Maybe select smaller time frame or try refreshing."
                data-bs-placement="bottom"
              >
                Timed Out
              </span>
            </Show>
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

function SortCaret(props: { order: "desc" | "asc" }) {
  return (
    <>
      <Show when={props.order === "desc"}>
        <BiCaretDownFill />
      </Show>
      <Show when={props.order === "asc"}>
        <BiCaretUpFill />
      </Show>
    </>
  );
}

function SortHeader(props: {
  title: string;
  key: string;
  sortBy: string;
  sortOrder: "asc" | "desc";
  class?: string;
  onclick: (key: string) => void;
}) {
  return (
    <>
      <th
        class={props.class}
        onclick={() => props.onclick(props.key)}
        style={"cursor: pointer;"}
      >
        {props.title}
        <Show when={props.sortBy === props.key}>
          <SortCaret order={props.sortOrder} />
        </Show>
      </th>
    </>
  );
}
