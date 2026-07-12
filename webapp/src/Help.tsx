// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createSignal, For, Match, Show, Suspense, Switch } from "solid-js";
import { Button, Modal, Spinner, Tab, Table, Tabs } from "solid-bootstrap";
import { closeHelp, showHelp } from "./Top";

import { createResource } from "solid-js";
import { getUpdateManifest, getVersion, SERVER_REVISION } from "./api";
import { compareVersions, isPrerelease } from "./version";
import { GIT_REV } from "./gitrev";

export function HelpModal() {
  const [tab, setTab] = createSignal("keyboard");
  return (
    <Modal show={showHelp()} onHide={closeHelp} size={"lg"} scrollable>
      <Modal.Body>
        <Tabs activeKey={tab()} onSelect={setTab}>
          <Tab eventKey="keyboard" title="Keyboard Shortcuts">
            <Keyboard />
          </Tab>
          <Tab eventKey="query" title="Search Queries">
            <QueryHelp />
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

function QueryExample(props: { query: string; description: string }) {
  return (
    <tr>
      <td class="font-monospace text-nowrap">{props.query}</td>
      <td>{props.description}</td>
    </tr>
  );
}

function QueryHelp() {
  return (
    <div class="app-query-help-modal">
      <p>
        Search terms are separated by spaces. Add more terms to narrow the
        result set. Use quotes when a value contains spaces.
      </p>

      <h6>Common Examples</h6>
      <Table bordered hover responsive size="sm">
        <tbody>
          <QueryExample
            query="dns"
            description="Find events containing the text dns."
          />
          <QueryExample
            query='"ET POLICY"'
            description="Find an exact quoted phrase."
          />
          <QueryExample
            query='dns -"et info"'
            description="Find dns events, excluding events containing et info."
          />
          <QueryExample
            query="src_ip:10.16.2.90"
            description="Match a specific field value."
          />
          <QueryExample
            query="event_type:alert app_proto:tls"
            description="Combine field filters."
          />
        </tbody>
      </Table>

      <h6>Shorthand Fields</h6>
      <Table bordered hover responsive size="sm">
        <tbody>
          <QueryExample
            query="@ip:10.16.1.10"
            description="Match source, destination, and known protocol IP fields."
          />
          <QueryExample
            query="@mac:00:11:22:33:44:55"
            description="Search known MAC-address content."
          />
          <QueryExample
            query="@sid:2030386"
            description="Match alert.signature_id."
          />
          <QueryExample
            query='@sig:"SURICATA STREAM"'
            description="Match alert.signature."
          />
        </tbody>
      </Table>

      <h6>Time Filters</h6>
      <Table bordered hover responsive size="sm">
        <tbody>
          <QueryExample
            query="@from:2026-07-02T18:00:00"
            description="Include events at or after this timestamp."
          />
          <QueryExample
            query="@to:2026-07-02T19:00:00"
            description="Include events at or before this timestamp."
          />
          <QueryExample
            query="@after:2026-07-02T18:00:00"
            description="Include events after this timestamp."
          />
          <QueryExample
            query="@before:2026-07-02T19:00:00"
            description="Include events before this timestamp."
          />
        </tbody>
      </Table>

      <h6>Alert State</h6>
      <Table bordered hover responsive size="sm">
        <tbody>
          <QueryExample
            query="is:archived"
            description="Show archived alerts."
          />
          <QueryExample
            query="-is:archived"
            description="Show alerts that are not archived."
          />
          <QueryExample
            query="is:escalated"
            description="Show escalated alerts."
          />
          <QueryExample
            query="-is:escalated"
            description="Show alerts that are not escalated."
          />
        </tbody>
      </Table>

      <h6>Negation</h6>
      <p>
        Prefix a term with <code>-</code> or <code>!</code> to exclude it.
        Negation works with free text, quoted phrases, field filters, shorthand
        fields, and alert-state filters.
      </p>
    </div>
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
    [then("g", "i"), "Go to inbox"],
    [then("g", "s"), "Go to escalated"],
    [then("g", "a"), "Go to alerts"],
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
    [plus("Shift", "h"), "Go to first row"],
    [plus("Shift", "g"), "Go to last row"],
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

type UpdateCheck = {
  status: "idle" | "checking" | "uptodate" | "development" | "update" | "error";
  latest?: string;
};

function About() {
  const [version] = createResource(getVersion);
  const [check, setCheck] = createSignal<UpdateCheck>({ status: "idle" });

  const checkForUpdates = async () => {
    const current = version()?.version;
    if (!current) {
      return;
    }
    setCheck({ status: "checking" });
    try {
      const manifest = await getUpdateManifest();
      const latest = manifest.version;
      const cmp = compareVersions(current, latest);
      if (cmp === null) {
        setCheck({ status: "error" });
      } else if (cmp < 0) {
        setCheck({ status: "update", latest });
      } else if (isPrerelease(current)) {
        setCheck({ status: "development", latest });
      } else {
        setCheck({ status: "uptodate", latest });
      }
    } catch (_e) {
      setCheck({ status: "error" });
    }
  };

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
          <Button
            variant="primary"
            size="sm"
            disabled={check().status === "checking" || !version()?.version}
            onClick={checkForUpdates}
          >
            <Show
              when={check().status === "checking"}
              fallback={"Check for updates"}
            >
              <Spinner animation="border" size="sm" /> Checking...
            </Show>
          </Button>
        </p>

        <Switch>
          <Match when={check().status === "update"}>
            <div class="alert alert-success">
              A new EveBox release is available:{" "}
              <strong>{check().latest}</strong>.
            </div>
          </Match>
          <Match when={check().status === "development"}>
            <div class="alert alert-info">
              You are running a development build. The latest stable release is{" "}
              <strong>{check().latest}</strong>.
            </div>
          </Match>
          <Match when={check().status === "uptodate"}>
            <div class="alert alert-secondary">EveBox is up to date.</div>
          </Match>
          <Match when={check().status === "error"}>
            <div class="alert alert-warning">
              Could not reach the update server.
            </div>
          </Match>
        </Switch>

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
