// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Container, Dropdown, Nav, Navbar, NavDropdown } from "solid-bootstrap";
import { A, useLocation, useNavigate, useSearchParams } from "@solidjs/router";
import { createEffect, createSignal, For, onMount, Show } from "solid-js";

import tinykeys from "tinykeys";
import { BiGear } from "./icons";
import { HelpModal } from "./Help";
import { QUEUE_SIZE, SERVER_REVISION } from "./api";
import { GIT_REV } from "./gitrev";
import { serverConfig, serverConfigSet } from "./config";

export const [showHelp, setShowHelp] = createSignal(false);
export const openHelp = () => setShowHelp(true);
export const closeHelp = () => setShowHelp(false);

const DEFAULT_TIME_RANGE = "24h";

const TIME_RANGE_OPTIONS = [
  ["60s", "Last Minute"],
  ["3h", "Last 3 Hours"],
  ["6h", "Last 6 Hours"],
  ["12h", "Last 12 Hours"],
  ["24h", "Last 24 Hours"],
  ["3d", "Last 3 Days"],
  ["7d", "Last Week"],
  ["all", "All"],
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
  if (localTimeRange && isValidTimeRange(localTimeRange)) {
    console.log(`Using local storage time range of ${localTimeRange}`);
    return localTimeRange;
  }

  const serverTimeRange = serverConfig?.defaults?.time_range;
  if (serverTimeRange && isValidTimeRange(serverTimeRange)) {
    console.log(`Using server side default time range of ${serverTimeRange}`);
    return serverTimeRange;
  }

  console.log(`Using default time range of ${DEFAULT_TIME_RANGE}`);
  return DEFAULT_TIME_RANGE;
}

/* START: Init TIME_RANGE */
export const [TIME_RANGE, SET_TIME_RANGE] =
  createSignal<string>(DEFAULT_TIME_RANGE);

export function Top(props: { brand?: string }) {
  console.log("Top");
  const navigate = useNavigate();
  const [_searchParams, setSearchParams] = useSearchParams();
  const brand = props.brand || "EveBox";

  // Control the state of the tool dropdown here to get around a SolidJS router issue where click on the active route
  // in the dropdown will not cause the dropdown to close.
  let [toolDropDownOpen, setToolDropDownOpen] = createSignal(false);

  SET_TIME_RANGE(getInitialTimeRange());

  function updateTimeRange(range: string) {
    SET_TIME_RANGE(range);
    localStorage.setItem("TIME_RANGE", range);
  }

  onMount(() => {
    tinykeys(window, {
      "Shift+?": () => {
        openHelp();
      },
      "g i": () => {
        navigate("/inbox");
      },
      "g a": () => {
        navigate("/alerts");
      },
      "g s": () => {
        navigate("/escalated");
      },
      "g e": () => {
        navigate("/events");
      },
      "Control+\\": () => {
        setSearchParams({ q: undefined });
      },
      "\\": () => {
        const e = document.getElementById("time-range-dropdown");
        console.log(e);
        if (e) {
          e.focus();
          e.click();
        }
      },
    });
  });

  createEffect(() => {
    for (let opt of TIME_RANGE_OPTIONS) {
      if (opt[0] === TIME_RANGE()) {
        document.getElementById("time-range-dropdown")!.innerHTML = opt[1]!;
      }
    }
  });

  return (
    <>
      <HelpModal />
      <Navbar collapseOnSelect expand="lg">
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
                <A href={"/reports/alerts"} class={"dropdown-item"}>
                  Alerts
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
                                updateTimeRange(o[0] as string);
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
              <Nav.Link onClick={openHelp}>Help</Nav.Link>
              <NavDropdown
                title={<BiGear />}
                align={"end"}
                show={toolDropDownOpen()}
                onclick={() => {
                  setToolDropDownOpen(!toolDropDownOpen());
                }}
              >
                <A href={"/settings"} class={"dropdown-item"}>
                  Settings
                </A>
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
