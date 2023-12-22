// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createSignal, For, Show, Suspense } from "solid-js";
import { Button, Modal, Tab, Tabs } from "solid-bootstrap";
import { closeHelp, showHelp } from "./Top";

import { createResource } from "solid-js";
import { getVersion, SERVER_REVISION } from "./api";
import { GIT_REV } from "./gitrev";

export function HelpModal() {
  const [tab, setTab] = createSignal("keyboard");
  return (
    <Modal show={showHelp()} onHide={closeHelp} size={"lg"}>
      <Modal.Body>
        <Tabs activeKey={tab()} onSelect={setTab}>
          <Tab
            eventKey="keyboard"
            title="Keyboard Shortcuts"
            class="help-tab"
            style={"padding: 0px !important; margin: 0px !important;"}
          >
            <Keyboard />
          </Tab>
          <Tab eventKey="about" title="About" class="app-help-tab">
            <About />
          </Tab>
        </Tabs>
      </Modal.Body>
      <Modal.Footer class="app-help-footer">
        <Button variant="secondary" onClick={closeHelp}>
          Close
        </Button>
      </Modal.Footer>
    </Modal>
  );
}

function Keyboard() {
  const shortcuts = [
    { s: "?", h: "Show help" },
    { s: "g i", h: "Goto inbox" },
    { s: "g s", h: "Goto escalated" },
    { s: "g a", h: "Goto alerts" },
    {
      s: "e",
      h: "Archive selected events, or event at cursor if none selected",
    },
    { s: "F8", h: "Archive event at cursor" },
    { s: "F9, Shift+s", h: "Escalate and archive event at cursor" },
    { s: "x", h: "Select event at cursor" },
    {
      s: "s",
      h: "Escalated selected events, or event at cursor if none selected",
    },
    { s: "j", h: "Move cursor to next event" },
    { s: "k", h: "Move cursor to previous event" },
    { s: ".", h: "Show action menu for event at cursor" },
    { s: "Control+\\", h: "Clear all filters and search" },
    { s: "Shift+h", h: "Goto first row" },
    { s: "Shift+g", h: "Goto last row" },
    { s: "* a", h: "Select all alerts in view" },
    { s: "* n", h: "Deselect all alerts" },
    { s: "* 1", h: "Select all alerts with current SID" },
  ];

  return (
    <>
      <table class={"table table-bordered table-sm mb-0"}>
        <tbody>
          <For each={shortcuts}>
            {(e, i) => (
              <>
                <tr>
                  <td
                    class={"shortcut"}
                    style={"white-space: nowrap !important;"}
                  >
                    {e.s}
                  </td>
                  <td>{e.h}</td>
                </tr>
              </>
            )}
          </For>
        </tbody>
      </table>
    </>
  );
}

function About() {
  const [version] = createResource(getVersion);

  return (
    <>
      <div style="padding: 12px">
        <p>
          <Suspense fallback={<>Loading version info...</>}>
            This is EveBox version {version()?.version} (Rev:{" "}
            {version()?.revision}).
          </Suspense>
        </p>

        <Show when={SERVER_REVISION() && SERVER_REVISION() != GIT_REV}>
          <div class={"alert alert-danger"}>
            Warning: The server and frontend versions differ. Please reload.
            <br />
            Server={SERVER_REVISION()}, Frontend={GIT_REV}.
          </div>
        </Show>

        <p>
          Homepage: <a href="https://evebox.org">https://evebox.org</a>
        </p>

        <p>
          GitHub:
          <a href="http://github.com/jasonish/evebox">
            http://github.com/jasonish/evebox
          </a>
        </p>
      </div>
    </>
  );
}
