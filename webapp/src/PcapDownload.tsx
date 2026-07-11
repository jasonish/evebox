// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  createEffect,
  createSignal,
  createUniqueId,
  For,
  onCleanup,
  Show,
} from "solid-js";
import { useSearchParams } from "@solidjs/router";
import {
  Button,
  Col,
  Container,
  Dropdown,
  Form,
  Row,
  Spinner,
} from "solid-bootstrap";
import { API, getEventById } from "./api";
import { serverConfig } from "./config";
import { EventSource } from "./types";
import { addError, addNotification } from "./Notifications";
import { Top } from "./Top";
import {
  datetime_local_to_rfc3339,
  is_valid_datetime_local,
  to_datetime_local_input,
} from "./datetime";

// The duration presets offered by the small dropdown next to a
// duration text field.
const DURATION_PRESETS = ["1m", "5m", "15m", "1h"];

// The max-size presets offered next to the max-size field.
// "unlimited" lifts the output cap entirely.
const MAX_SIZE_PRESETS = ["50mb", "200mb", "1gb", "unlimited"];

// Format a byte count the way the max-size field expects it back
// ("50mb", "2gb", or a bare byte count), using the same decimal units
// as the server's size parser.
function formatMaxSize(bytes: number): string {
  if (bytes > 0 && bytes % 1_000_000_000 === 0) {
    return `${bytes / 1_000_000_000}gb`;
  }
  if (bytes > 0 && bytes % 1_000_000 === 0) {
    return `${bytes / 1_000_000}mb`;
  }
  return String(bytes);
}

// The server's default pcap max size, pre-filled into the field so the
// user sees the effective cap. Empty when the server did not report one
// (pcap compiled out); an empty field falls back to the server default.
function defaultMaxSize(): string {
  const bytes = serverConfig()?.pcap?.max_size_bytes;
  return bytes != null ? formatMaxSize(bytes) : "";
}

// Map a structured pcap error to a user-facing message. Shared by the
// event view download button and the standalone page.
export function pcapErrorMessage(err: any): string {
  switch (err.code) {
    case "no-match":
      return "No packets matched this event's flow in the capture.";
    case "no-candidate-files":
      return "No capture files cover this time window (rotated out).";
    case "busy":
      return "Packet capture is busy; try again shortly.";
    case "timeout":
      return "Packet capture did not produce output in time.";
    case "bad-filter":
      return `Invalid capture filter: ${err.message}`;
    case "not-configured":
      return "Packet capture is not configured.";
    default:
      return err.message ?? "PCAP request failed.";
  }
}

// A tiny download controller for the standalone page. It pre-flights
// the request (validate), surfacing request errors, then hands the byte
// transfer to the browser to stream straight to disk. Starting a new
// download aborts any pre-flight still in flight; unmounting cancels
// only that pre-flight. The browser owns the native transfer after it
// starts.
function usePcapDownload(): {
  pending: () => boolean;
  download: (params: API.PcapRequestParams) => Promise<void>;
  cancel: () => void;
} {
  const [pending, setPending] = createSignal(false);
  let abort: AbortController | undefined;

  const download = async (params: API.PcapRequestParams) => {
    // A new request supersedes any pre-flight still in flight.
    abort?.abort();
    const controller = new AbortController();
    abort = controller;
    setPending(true);
    try {
      // Pre-flight so a bad request surfaces as a toast; then hand the
      // transfer to the browser, which streams it straight to disk.
      await API.validatePcap(params, controller.signal);
      // The streamed download has no in-page progress, and an empty
      // result is saved as a valid empty pcap rather than an error, so
      // set expectations before handing off to the browser.
      addNotification("Capture starting; your download will begin shortly");
      API.startPcapDownload(params, (error) => {
        addError(pcapErrorMessage(error));
      });
    } catch (err: any) {
      if (err?.name === "AbortError") {
        // User cancelled (or navigated away); not an error.
      } else if (err instanceof API.PcapError) {
        addError(pcapErrorMessage(err));
      } else {
        addError(`PCAP request failed: ${err}`);
      }
    } finally {
      // A superseded request must not clear the newer request's state.
      if (abort === controller) {
        abort = undefined;
        setPending(false);
      }
    }
  };

  const cancel = () => {
    abort?.abort();
    abort = undefined;
    setPending(false);
  };

  onCleanup(cancel);

  return { pending, download, cancel };
}

// Build an editable BPF approximation of an event's flow, client-side,
// from the event's _source fields. This is only a starting point the
// user can edit or clear.
// BPF primitives for the protocol names Suricata reports. Protocols
// without a BPF keyword (GRE, ESP, ...) are left out of the filter; the
// host clauses still narrow the match.
const BPF_PROTO_KEYWORDS: { [proto: string]: string } = {
  tcp: "tcp",
  udp: "udp",
  sctp: "sctp",
  icmp: "icmp",
  "ipv6-icmp": "icmp6",
  icmpv6: "icmp6",
};

// Protocols whose events carry real ports. Suricata reuses the port
// fields for the ICMP type and code, which a BPF "port" clause would
// reject.
const PORT_PROTOCOLS = ["tcp", "udp", "sctp"];

function buildFlowFilter(source: EventSource): string {
  const parts: string[] = [];
  const proto = source.proto?.toLowerCase();
  if (proto && BPF_PROTO_KEYWORDS[proto]) {
    parts.push(BPF_PROTO_KEYWORDS[proto]);
  }
  if (source.src_ip) {
    parts.push(`host ${source.src_ip}`);
  }
  if (source.dest_ip) {
    parts.push(`host ${source.dest_ip}`);
  }
  if (proto && PORT_PROTOCOLS.includes(proto)) {
    if (source.src_port !== undefined && source.src_port !== null) {
      parts.push(`port ${source.src_port}`);
    }
    if (source.dest_port !== undefined && source.dest_port !== null) {
      parts.push(`port ${source.dest_port}`);
    }
  }
  return parts.join(" and ");
}

// A labelled text field with a small "Preset" dropdown that fills in
// one of a few common values. Shared by the duration and max-size
// fields.
function PresetTextField(props: {
  label: string;
  value: string;
  setValue: (v: string) => void;
  presets: string[];
  placeholder: string;
}) {
  const controlId = createUniqueId();
  return (
    <Form.Group class={"mb-3"} controlId={controlId}>
      <Form.Label>{props.label}</Form.Label>
      <div class={"d-flex gap-2"}>
        <Form.Control
          type={"text"}
          value={props.value}
          onInput={(e) => props.setValue(e.currentTarget.value)}
          placeholder={props.placeholder}
        />
        <Dropdown>
          <Dropdown.Toggle
            variant={"outline-secondary"}
            aria-label={`${props.label} presets`}
          >
            Preset
          </Dropdown.Toggle>
          <Dropdown.Menu align={"end"}>
            <For each={props.presets}>
              {(p) => (
                <Dropdown.Item onClick={() => props.setValue(p)}>
                  {p}
                </Dropdown.Item>
              )}
            </For>
          </Dropdown.Menu>
        </Dropdown>
      </div>
    </Form.Group>
  );
}

// A duration text field ("1m", "5m", ...) with a preset dropdown.
function DurationField(props: {
  label: string;
  value: string;
  setValue: (v: string) => void;
}) {
  return (
    <PresetTextField
      label={props.label}
      value={props.value}
      setValue={props.setValue}
      presets={DURATION_PRESETS}
      placeholder={"1m"}
    />
  );
}

// A max-size text field ("200mb", "1gb", "unlimited", ...) with a
// preset dropdown. An empty value leaves the server's configured
// default in place.
function MaxSizeField(props: { value: string; setValue: (v: string) => void }) {
  return (
    <PresetTextField
      label={"Max size"}
      value={props.value}
      setValue={props.setValue}
      presets={MAX_SIZE_PRESETS}
      placeholder={"server default"}
    />
  );
}

// The primary action button: it starts validation when idle and cancels
// while that pre-flight is pending. The browser owns the native transfer.
function PcapDownloadButton(props: {
  pending: boolean;
  disabled?: boolean;
  onDownload: () => void;
  onCancel: () => void;
}) {
  return (
    <Button
      variant={"primary"}
      disabled={!props.pending && props.disabled}
      onClick={() => (props.pending ? props.onCancel() : props.onDownload())}
    >
      <Show when={props.pending} fallback={"Download"}>
        <Spinner
          as={"span"}
          animation={"border"}
          size={"sm"}
          role={"status"}
          aria-hidden={"true"}
        />{" "}
        Cancel…
      </Show>
    </Button>
  );
}

// The free-form PCAP filter form fields (filter, optional "use this
// flow", start time, duration). The page owns the signals and may
// provide an event as a convenient starting point.
function PcapFilterFields(props: {
  filter: string;
  setFilter: (v: string) => void;
  start: string;
  setStart: (v: string) => void;
  duration: string;
  setDuration: (v: string) => void;
  maxSize: string;
  setMaxSize: (v: string) => void;
  // When present, offers a "Use this flow" button that fills the
  // filter with an editable BPF approximation of the event's flow.
  event?: EventSource;
}) {
  const filterId = createUniqueId();
  const startId = createUniqueId();
  return (
    <>
      <Form.Group class={"mb-3"} controlId={filterId}>
        <Form.Label>PCAP filter</Form.Label>
        <Form.Control
          as={"textarea"}
          rows={2}
          class={"font-monospace"}
          value={props.filter}
          onInput={(e) => props.setFilter(e.currentTarget.value)}
          placeholder={"PCAP filter… (empty matches all packets)"}
        />
        <Show when={props.event}>
          <Button
            variant={"link"}
            size={"sm"}
            class={"p-0 mt-1 text-decoration-none"}
            onClick={() => props.setFilter(buildFlowFilter(props.event!))}
          >
            ↳ Use this event's flow
          </Button>
        </Show>
      </Form.Group>
      <Form.Group class={"mb-3"} controlId={startId}>
        <Form.Label>Start time</Form.Label>
        <Form.Control
          type={"datetime-local"}
          step={1}
          value={props.start}
          onInput={(e) => props.setStart(e.currentTarget.value)}
        />
      </Form.Group>
      <DurationField
        label={"Duration"}
        value={props.duration}
        setValue={props.setDuration}
      />
      <MaxSizeField value={props.maxSize} setValue={props.setMaxSize} />
    </>
  );
}

// Standalone custom-download page. With an event_id query parameter it
// loads that event and pre-fills the editable start time and BPF. With
// no event context it starts as a generic time-and-filter form.
export function PcapDownload() {
  const [searchParams] = useSearchParams<{ event_id?: string }>();
  const dl = usePcapDownload();
  const [eventContext, setEventContext] = createSignal<EventSource>();
  const [eventContextLoading, setEventContextLoading] = createSignal(false);
  const [eventContextFailed, setEventContextFailed] = createSignal(false);
  const [filter, setFilter] = createSignal("");
  const [start, setStart] = createSignal(to_datetime_local_input());
  const [duration, setDuration] = createSignal("1m");
  const [maxSize, setMaxSize] = createSignal(defaultMaxSize());
  let requestedEventId: string | undefined;

  const eventId = () => searchParams.event_id?.trim() || undefined;

  createEffect(() => {
    const id = eventId();
    requestedEventId = id;
    setEventContext(undefined);
    setEventContextFailed(false);

    if (!id) {
      setEventContextLoading(false);
      setFilter("");
      setStart(to_datetime_local_input());
      return;
    }

    setEventContextLoading(true);
    getEventById(id)
      .then((event) => {
        if (requestedEventId !== id) return;
        setEventContext(event._source);
        setFilter(buildFlowFilter(event._source));
        setStart(
          to_datetime_local_input(
            event._source.flow?.start || event._source.timestamp,
          ),
        );
      })
      .catch((err: any) => {
        if (requestedEventId !== id) return;
        console.log(`Failed to load event context for PCAP download: ${err}`);
        setEventContextFailed(true);
      })
      .finally(() => {
        if (requestedEventId === id) {
          setEventContextLoading(false);
        }
      });
  });

  const download = () => {
    if (!is_valid_datetime_local(start())) {
      addError("Enter a valid start time.");
      return;
    }
    const params: API.PcapRequestParams = {
      start: datetime_local_to_rfc3339(start()),
      duration: duration() || "1m",
    };
    const contextEventId = eventId();
    if (eventContext() && contextEventId) {
      params.eventId = contextEventId;
    }
    if (filter().trim() !== "") {
      params.filter = filter();
    }
    if (maxSize().trim() !== "") {
      params.maxSize = maxSize();
    }
    dl.download(params);
  };

  return (
    <>
      <Top />
      <Container class={"mt-4"}>
        <Row class={"justify-content-center"}>
          <Col sm={12} md={10} lg={8} xl={6}>
            <div class={"card"}>
              <div class={"card-header"}>Download PCAP</div>
              <div class={"card-body"}>
                <Show
                  when={serverConfig()?.pcap != null}
                  fallback={
                    <div class={"alert alert-secondary mb-0"}>
                      Packet capture is not configured.
                    </div>
                  }
                >
                  <Show when={eventContextLoading()}>
                    <div class={"alert alert-secondary"}>
                      Loading event context…
                    </div>
                  </Show>
                  <Show when={eventContextFailed()}>
                    <div class={"alert alert-warning"}>
                      Could not load event {eventId()}; using a generic custom
                      download instead.
                    </div>
                  </Show>
                  <Show when={eventContext()}>
                    <div class={"alert alert-secondary"}>
                      Start time and filter were pre-filled from event{" "}
                      {eventId()}.
                    </div>
                  </Show>
                  <PcapFilterFields
                    filter={filter()}
                    setFilter={setFilter}
                    start={start()}
                    setStart={setStart}
                    duration={duration()}
                    setDuration={setDuration}
                    maxSize={maxSize()}
                    setMaxSize={setMaxSize}
                    event={eventContext()}
                  />
                  <PcapDownloadButton
                    pending={dl.pending()}
                    disabled={
                      eventContextLoading() || !is_valid_datetime_local(start())
                    }
                    onDownload={download}
                    onCancel={dl.cancel}
                  />
                </Show>
              </div>
            </div>
          </Col>
        </Row>
      </Container>
    </>
  );
}
