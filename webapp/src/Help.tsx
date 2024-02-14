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
          <Tab eventKey="keyboard" title="Keyboard Shortcuts">
            <Keyboard />
          </Tab>
          <Tab eventKey="about" title="About">
            <About />
          </Tab>
        </Tabs>
      </Modal.Body>
      <Modal.Footer>
        <Button variant="secondary" onClick={closeHelp}>
          Close
        </Button>
      </Modal.Footer>
    </Modal>
  );
}

function Keyboard() {
  let key = (k: string) => {
    return <span class="font-monospace">{k}</span>;
  };

  let then = (a: string, b: string) => {
    return (
      <>
        {key(a)} <span class="fw-lighter">then</span> {key(b)}
      </>
    );
  };

  let plus = (a: string, b: string) => {
    return (
      <>
        {key(a)} <span class="fw-lighter">+</span> {key(b)}
      </>
    );
  };

  const shortcuts = [
    [key("?"), "Show help"],
    [then("g", "i"), "Goto inbox"],
    [then("g", "s"), "Goto escalated"],
    [then("g", "a"), "Goto alerts"],
    [key("e"), "Archive selected events, or event at cursor if none selected"],
    [key("F8"), "Archive event at cursor"],
    [plus("Shift", "s"), "Escalate and archive event at cursor"],
    [key("F9"), "Escalate and archive event at cursor"],
    [key("x"), "Select event at cursor"],
    [key("s"), "Escalate selected events, or event at cursor if none selected"],
    [key("j"), "Move cursor to next event"],
    [key("k"), "Move cursor to previous event"],
    [key("."), "Show action menu for event at cursor"],
    [plus("Control", "\\"), "Clear all filters and search"],
    [plus("Shift", "h"), "Goto first row"],
    [plus("Shift", "g"), "Goto last row"],
    [then("*", "a"), "Select all alerts in view"],
    [then("*", "n"), "Deselect all alerts"],
    [then("*", "1"), "Select all alerts with current SID"],
  ];

  return (
    <>
      <p></p>
      <table class={"table table-bordered table-sm p-5"}>
        <tbody class="p-5">
          <For each={shortcuts}>
            {(e, i) => (
              <>
                <tr>
                  <td style={"white-space: nowrap !important;"}>{e[0]}</td>
                  <td>{e[1]}</td>
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
          Homepage and Documentation:{" "}
          <a href="https://evebox.org">https://evebox.org</a>
        </p>

        <p>
          GitHub:{" "}
          <a href="http://github.com/jasonish/evebox">
            http://github.com/jasonish/evebox
          </a>
        </p>
      </div>
    </>
  );
}
