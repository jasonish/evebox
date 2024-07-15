// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Container, Dropdown, Nav, Navbar, NavDropdown } from "solid-bootstrap";
import { A, useNavigate, useSearchParams } from "@solidjs/router";
import { createEffect, createSignal, For, onMount, Show } from "solid-js";

import tinykeys from "tinykeys";
import { BiGear } from "./icons";
import { HelpModal } from "./Help";
import { QUEUE_SIZE, SERVER_REVISION } from "./api";
import * as api from "./api";
import { GIT_REV } from "./gitrev";
import { serverConfig } from "./config";
import { IS_AUTHENTICATED } from "./global";

export const [showHelp, setShowHelp] = createSignal(false);
export const openHelp = () => setShowHelp(true);
export const closeHelp = () => setShowHelp(false);

const DEFAULT_TIME_RANGE = "24h";

const TIME_RANGE_OPTIONS = [
  ["60s", "Last Minute"],
  ["1h", "Last 1 Hour"],
  ["3h", "Last 3 Hours"],
  ["6h", "Last 6 Hours"],
  ["12h", "Last 12 Hours"],
  ["24h", "Last 24 Hours"],
  ["3d", "Last 3 Days"],
  ["7d", "Last Week"],
  ["", "All"],
];

function isValidTimeRange(range: string): boolean {
  for (const tr of TIME_RANGE_OPTIONS) {
    if (tr[0] === range) {
      return true;
    }
  }
  return false;
}

function getInitialTimeRange(): string {
  const localTimeRange = localStorage.getItem("TIME_RANGE");
  console.log(`Time-range from localStorage:`);
  console.log(localTimeRange);

  if (localTimeRange === "") {
    console.log('localTimeRange is an empty string, returning ""');
    return "";
  }

  if (localTimeRange && isValidTimeRange(localTimeRange)) {
    console.log(`Using local storage time range of ${localTimeRange}`);
    return localTimeRange;
  }

  const serverTimeRange = serverConfig?.defaults?.time_range;
  console.log("serverTimeRange:");
  console.log(serverTimeRange);
  if (serverTimeRange && isValidTimeRange(serverTimeRange)) {
    console.log(`Using server side default time range of ${serverTimeRange}`);
    return serverTimeRange;
  } else if (serverTimeRange && serverTimeRange === "all") {
    // The server time range might be "all" which is invalid in the front-end,
    // but it does imply that the time-range should be all. Needs to be cleaned up.
    return "";
  }

  console.log(`Using default time range of ${DEFAULT_TIME_RANGE}`);
  return DEFAULT_TIME_RANGE;
}

/* START: Init TIME_RANGE */
export const [TIME_RANGE, _SET_TIME_RANGE] =
  createSignal<string>(DEFAULT_TIME_RANGE);

export function SET_TIME_RANGE(range: string) {
  console.log(`Setting localStorage TIME_RANGE to ${range}`);
  _SET_TIME_RANGE(range);
  localStorage.setItem("TIME_RANGE", range);
}

export function Top(props: { brand?: string; disableRange?: boolean }) {
  console.log("Top");
  console.log(`Top: disableRange=${props.disableRange}`);
  const navigate = useNavigate();
  const [_searchParams, setSearchParams] = useSearchParams();
  const brand = props.brand || "EveBox";

  // Control the state of the tool dropdown here to get around a SolidJS router issue where click on the active route
  // in the dropdown will not cause the dropdown to close.
  let [toolDropDownOpen, setToolDropDownOpen] = createSignal(false);

  _SET_TIME_RANGE(getInitialTimeRange());

  onMount(() => {
    tinykeys(window, {
      "Shift+?": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        openHelp();
      },
      "g i": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        navigate("/inbox");
      },
      "g a": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        navigate("/alerts");
      },
      "g s": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        navigate("/escalated");
      },
      "g e": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        navigate("/events");
      },
      "Control+\\": (e: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) {
          return;
        }
        setSearchParams({ q: undefined });
      },
      "\\": (event: any) => {
        if (["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName)) {
          return;
        }
        const e = document.getElementById("time-range-dropdown");
        if (e) {
          e.focus();
          e.click();
        }
      },
    });
  });

  createEffect(() => {
    if (!props.disableRange) {
      for (let opt of TIME_RANGE_OPTIONS) {
        if (opt[0] === TIME_RANGE()) {
          document.getElementById("time-range-dropdown")!.innerHTML = opt[1]!;
        }
      }
    }
  });

  async function logout() {
    await api.logout();
    navigate("/");
  }

  return (
    <>
      <HelpModal />
      <Navbar collapseOnSelect expand="lg" class="bg-body-tertiary">
        <Container fluid>
          <Navbar.Brand href="/">{brand}</Navbar.Brand>
          <Navbar.Toggle />
          <Navbar.Collapse id="responsive-navbar-nav">
            <Nav class="me-auto">
              <Nav.Item>
                <A href={"/inbox"} class={"nav-link"}>
                  Inbox
                </A>
              </Nav.Item>
              <Nav.Item>
                <A href={"/escalated"} class={"nav-link"}>
                  Escalated
                </A>
              </Nav.Item>
              <Nav.Item>
                <A href={"/alerts"} class={"nav-link"}>
                  Alerts
                </A>
              </Nav.Item>
              <Nav.Item>
                <A href={"/stats"} class={"nav-link"}>
                  Stats
                </A>
              </Nav.Item>
              <Nav.Item>
                <A href={"/events"} class={"nav-link"}>
                  Events
                </A>
              </Nav.Item>
              <NavDropdown
                title="Reports"
                active={location.pathname.startsWith("/reports")}
              >
                <A href={"/reports/overview"} class={"dropdown-item"}>
                  Overview
                </A>
                <A href={"/reports/alerts"} class={"dropdown-item"}>
                  Alerts
                </A>
                <A href={"/reports/dhcp"} class={"dropdown-item"}>
                  DHCP
                </A>
              </NavDropdown>
            </Nav>
            <Nav>
              <Show
                when={SERVER_REVISION() != null && SERVER_REVISION() != GIT_REV}
              >
                <Nav.Item>
                  <button
                    type={"button"}
                    class={"btn btn-danger me-2"}
                    onclick={() => window.location.reload()}
                  >
                    Reload Required
                  </button>
                </Nav.Item>
              </Show>
              <Show when={!props.disableRange}>
                <Nav.Item>
                  {
                    // A dropdown is used here instead of a traditional select as it allows us to drop it down
                    // with a keyboard shortcut.
                  }
                  <Dropdown>
                    <Dropdown.Toggle
                      id={"time-range-dropdown"}
                      variant="outline-secondary"
                      style={"width: 9em;"}
                    ></Dropdown.Toggle>
                    <Dropdown.Menu>
                      <For each={TIME_RANGE_OPTIONS}>
                        {(o) => {
                          return (
                            <>
                              <Dropdown.Item
                                onClick={(e) => {
                                  SET_TIME_RANGE(o[0] as string);
                                  e.currentTarget.blur();
                                }}
                              >
                                {o[1]}
                              </Dropdown.Item>
                            </>
                          );
                        }}
                      </For>
                    </Dropdown.Menu>
                  </Dropdown>
                </Nav.Item>
              </Show>
              <Nav.Link onClick={openHelp}>Help</Nav.Link>
              <NavDropdown
                title={<BiGear />}
                align={"end"}
                show={toolDropDownOpen()}
                onclick={() => {
                  setToolDropDownOpen(!toolDropDownOpen());
                }}
              >
                <A href="/settings" class="dropdown-item">
                  Settings
                </A>
                <A href="/admin" class="dropdown-item">
                  Admin
                </A>
                <Show when={IS_AUTHENTICATED()}>
                  <a class="dropdown-item" onClick={logout}>
                    Logout
                  </a>
                </Show>
              </NavDropdown>
              <Nav.Item>
                <button
                  type={"button"}
                  class={"btn btn-secondary btn-sm"}
                  style={"margin-top: 5px !important;"}
                >
                  {QUEUE_SIZE()}
                </button>
              </Nav.Item>
            </Nav>
          </Navbar.Collapse>
        </Container>
      </Navbar>
    </>
  );
}
