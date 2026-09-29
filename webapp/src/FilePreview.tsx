// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// In-app preview of files extracted by Suricata's file-store output.
//
// Extracted files are untrusted. The preview only ever fetches bytes
// into memory and shows derived text (hex, decoded text, strings) as
// Solid text nodes: file bytes are never turned into a URL, placed in a
// media or frame element, or inserted as HTML, and nothing about the
// file is sent anywhere but the EveBox server.

import {
  createMemo,
  createSignal,
  For,
  Match,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { Button, Modal, Nav, Spinner } from "solid-bootstrap";
import { API } from "./api";
import { downloadFile, EventFile, fileErrorMessage } from "./FileDownload";
import { formatBytes } from "./formatters";
import { EventSource } from "./types";
import { hexLines } from "./fileview/hex";
import {
  detectType,
  entropy,
  isProbablyText,
  textLabel,
} from "./fileview/filetype";
import {
  decodeText,
  TextEncodingChoice,
  visibleControls,
} from "./fileview/text";
import {
  classify,
  extractStrings,
  STRING_TAGS,
  StringTag,
} from "./fileview/strings";
import { fileWarnings } from "./fileview/warnings";

// Hex lines per <pre> block.
const HEX_BLOCK_LINES = 256;
// Most string rows rendered at once; filter to see others.
const MAX_STRING_ROWS = 2000;
// Entropy above which data is likely compressed, encrypted or packed.
const HIGH_ENTROPY = 7.2;

type Tab = "info" | "hex" | "text" | "strings";

// The event's record for a file: its fileinfo, or the matching entry of
// an alert's files array.
function fileRecord(
  source: EventSource | undefined,
  sha256: string,
): any | undefined {
  const matches = (entry: any) =>
    typeof entry?.sha256 === "string" && entry.sha256.toLowerCase() === sha256;
  if (matches(source?.fileinfo)) return source!.fileinfo;
  const files = (source as any)?.files;
  if (Array.isArray(files)) return files.find(matches);
  return undefined;
}

function errorText(err: unknown): string {
  if (err instanceof API.FileDownloadError) return fileErrorMessage(err);
  return `File preview failed: ${err}`;
}

async function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard) {
    return navigator.clipboard.writeText(text);
  }
  // navigator.clipboard only exists in secure contexts.
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!ok) throw new Error("copy failed");
}

function CopyButton(props: { text: string }) {
  const [state, setState] = createSignal<"idle" | "copied" | "failed">("idle");
  const flash = (result: "copied" | "failed") => {
    setState(result);
    setTimeout(() => setState("idle"), 2000);
  };
  return (
    <button
      type="button"
      class={
        "btn btn-sm py-0 " +
        (state() === "failed" ? "btn-outline-danger" : "btn-outline-secondary")
      }
      onClick={() =>
        copyToClipboard(props.text)
          .then(() => flash("copied"))
          .catch(() => flash("failed"))
      }
    >
      {state() === "copied"
        ? "Copied"
        : state() === "failed"
          ? "Copy failed"
          : "Copy"}
    </button>
  );
}

export function FilePreviewModal(props: {
  eventId?: string;
  file: EventFile | null;
  event?: EventSource;
  onClose: () => void;
}) {
  return (
    <Modal
      show={props.file !== null}
      onHide={props.onClose}
      size={"xl"}
      scrollable
    >
      <Show when={props.file} keyed>
        {(file) => (
          <PreviewContent
            eventId={props.eventId}
            file={file}
            event={props.event}
            onClose={props.onClose}
          />
        )}
      </Show>
    </Modal>
  );
}

function PreviewContent(props: {
  eventId?: string;
  file: EventFile;
  event?: EventSource;
  onClose: () => void;
}) {
  const [data, setData] = createSignal<Uint8Array>(new Uint8Array(0));
  const [total, setTotal] = createSignal<number | null>(null);
  const [source, setSource] = createSignal<string>("");
  const [error, setError] = createSignal<string | null>(null);
  const [tab, setTab] = createSignal<Tab | null>(null);
  const [downloading, setDownloading] = createSignal(false);

  const controller = new AbortController();
  onCleanup(() => controller.abort());

  const record = createMemo(() => fileRecord(props.event, props.file.sha256));
  const size = () => total() ?? record()?.size ?? props.file.size;
  // The server only returns the start of the file.
  const allLoaded = () => total() !== null && data().length >= total()!;

  onMount(async () => {
    try {
      const preview = await API.previewFile(
        { eventId: props.eventId, sha256: props.file.sha256 },
        controller.signal,
      );
      setSource(preview.source);
      setTotal(preview.total);
      setData(preview.bytes);
      setTab(isProbablyText(preview.bytes) ? "text" : "info");
    } catch (err: any) {
      if (err?.name === "AbortError") return;
      setError(errorText(err));
    }
  });

  const download = async () => {
    setDownloading(true);
    try {
      await downloadFile(props.eventId, props.file);
    } finally {
      setDownloading(false);
    }
  };

  const title = () =>
    props.file.filename || `${props.file.sha256.slice(0, 16)}…`;

  return (
    <>
      <Modal.Header closeButton>
        <div class="w-100 overflow-hidden">
          <Modal.Title as="h5" class="text-break">
            {title()}
          </Modal.Title>
          <div class="small text-muted">
            <Show when={size() !== undefined}>
              <div>{formatBytes(size()!, true)}</div>
            </Show>
            <HashLine name="SHA256" value={props.file.sha256} />
            <Show when={typeof record()?.md5 === "string"}>
              <HashLine name="MD5" value={record().md5} />
            </Show>
            <Show when={typeof record()?.sha1 === "string"}>
              <HashLine name="SHA1" value={record().sha1} />
            </Show>
          </div>
        </div>
      </Modal.Header>
      <Modal.Body>
        <Switch>
          <Match when={error()}>
            <div class="alert alert-danger mb-0">{error()}</div>
          </Match>
          <Match when={tab() === null}>
            <div class="text-center p-3">
              <Spinner animation="border" role="status" />
            </div>
          </Match>
          <Match when={tab()}>
            <Nav
              variant="tabs"
              class="mb-2"
              activeKey={tab()!}
              onSelect={(key) => key && setTab(key as Tab)}
            >
              <Nav.Item>
                <Nav.Link eventKey="info">Info</Nav.Link>
              </Nav.Item>
              <Nav.Item>
                <Nav.Link eventKey="hex">Hex</Nav.Link>
              </Nav.Item>
              <Nav.Item>
                <Nav.Link eventKey="text">Text</Nav.Link>
              </Nav.Item>
              <Nav.Item>
                <Nav.Link eventKey="strings">Strings</Nav.Link>
              </Nav.Item>
            </Nav>
            <Switch>
              <Match when={tab() === "info"}>
                <InfoView
                  data={data()}
                  allLoaded={allLoaded()}
                  source={source()}
                  record={record()}
                  event={props.event}
                />
              </Match>
              <Match when={tab() === "hex"}>
                <HexView data={data()} total={total()} />
              </Match>
              <Match when={tab() === "text"}>
                <TextView data={data()} total={total()} />
              </Match>
              <Match when={tab() === "strings"}>
                <StringsView data={data()} allLoaded={allLoaded()} />
              </Match>
            </Switch>
          </Match>
        </Switch>
      </Modal.Body>
      <Modal.Footer>
        <Show when={!error() && tab() !== null}>
          <span class="text-muted me-auto">
            <Show
              when={!allLoaded()}
              fallback={<>All {data().length.toLocaleString()} bytes</>}
            >
              First {data().length.toLocaleString()} of{" "}
              {total()!.toLocaleString()} bytes; download the file to see the
              rest.
            </Show>
          </span>
        </Show>
        <Button variant="primary" disabled={downloading()} onClick={download}>
          <Show when={downloading()}>
            <Spinner
              as="span"
              animation="border"
              size="sm"
              aria-hidden="true"
            />{" "}
          </Show>
          Download
        </Button>
        <Button variant="secondary" onClick={props.onClose}>
          Close
        </Button>
      </Modal.Footer>
    </>
  );
}

function HashLine(props: { name: string; value: string }) {
  return (
    <div class="d-flex align-items-center gap-2">
      <span>{props.name}:</span>
      <span class="font-monospace text-break">{props.value}</span>
      <CopyButton text={props.value} />
    </div>
  );
}

function InfoView(props: {
  data: Uint8Array;
  allLoaded: boolean;
  source: string;
  record: any;
  event?: EventSource;
}) {
  const detected = createMemo(() => detectType(props.data));
  const isText = createMemo(() => isProbablyText(props.data));
  const bits = createMemo(() => entropy(props.data));
  const http = () => props.event?.http as any;

  const warnings = createMemo(() =>
    fileWarnings({
      filename: props.record?.filename,
      contentType:
        typeof http()?.http_content_type === "string"
          ? http().http_content_type
          : undefined,
      magic:
        typeof props.record?.magic === "string"
          ? props.record.magic
          : undefined,
      gaps: props.record?.gaps,
      state: props.record?.state,
      detected: detected(),
      isText: isText(),
    }),
  );

  // Event fields, shown when present; values are displayed as text.
  const eventFields = createMemo(() => {
    const fields: [string, string][] = [];
    const add = (name: string, value: any) => {
      if (value !== undefined && value !== null && value !== "") {
        fields.push([name, String(value)]);
      }
    };
    const r = props.record;
    add("fileinfo.filename", r?.filename);
    add("fileinfo.size", r?.size);
    add("fileinfo.state", r?.state);
    add("fileinfo.stored", r?.stored);
    add("fileinfo.gaps", r?.gaps);
    add("fileinfo.magic", r?.magic);
    add("fileinfo.md5", r?.md5);
    add("fileinfo.sha1", r?.sha1);
    add("fileinfo.sha256", r?.sha256);
    add("http.http_content_type", http()?.http_content_type);
    add("http.hostname", http()?.hostname);
    add("http.url", http()?.url);
    return fields;
  });

  const ofWhat = () =>
    props.allLoaded
      ? ""
      : ` (of first ${props.data.length.toLocaleString()} bytes)`;

  return (
    <>
      <For each={warnings()}>
        {(warning) => <div class="alert alert-warning py-2">{warning}</div>}
      </For>
      <table class="table table-sm mb-3">
        <tbody>
          <tr>
            <th class="text-nowrap" style="width: 1%">
              Detected type
            </th>
            <td>
              {detected()?.label ??
                (isText() ? textLabel(props.data) : "Unknown")}
            </td>
          </tr>
          <tr>
            <th class="text-nowrap">Content</th>
            <td>
              {isText() ? "Text" : "Binary"}
              {props.allLoaded
                ? ""
                : ` (from first ${Math.min(props.data.length, 8192).toLocaleString()} bytes)`}
            </td>
          </tr>
          <tr>
            <th class="text-nowrap">Entropy</th>
            <td>
              {bits().toFixed(1)} bits/byte{ofWhat()}
              <Show when={bits() > HIGH_ENTROPY}>
                <span class="text-muted">
                  {" "}
                  — likely compressed, encrypted or packed
                </span>
              </Show>
            </td>
          </tr>
          <tr>
            <th class="text-nowrap">Source</th>
            <td>{props.source || "-"}</td>
          </tr>
        </tbody>
      </table>
      <Show when={eventFields().length > 0}>
        <h6>Event</h6>
        <table class="table table-sm mb-0">
          <tbody>
            <For each={eventFields()}>
              {([name, value]) => (
                <tr>
                  <th class="text-nowrap" style="width: 1%">
                    {name}
                  </th>
                  <td class="text-break">{value}</td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </Show>
    </>
  );
}

// Marks where the shown bytes stop, so the file's content is never
// confused with the UI around it.
function ContentEnd(props: { data: Uint8Array; total: number | null }) {
  return (
    <div class="card-footer py-1 small text-muted">
      <Show
        when={props.total !== null && props.data.length < props.total}
        fallback={<>End of file</>}
      >
        End of preview: first {props.data.length.toLocaleString()} of{" "}
        {props.total!.toLocaleString()} bytes
      </Show>
    </div>
  );
}

function HexView(props: { data: Uint8Array; total: number | null }) {
  // One <pre> per block of lines keeps the DOM small.
  const blocks = createMemo(() => {
    const lines = hexLines(props.data, 0);
    const out: string[] = [];
    for (let i = 0; i < lines.length; i += HEX_BLOCK_LINES) {
      out.push(lines.slice(i, i + HEX_BLOCK_LINES).join("\n"));
    }
    return out;
  });
  return (
    <div class="card">
      <div class="card-body app-file-content font-monospace small">
        <For each={blocks()}>
          {(block) => <pre class="app-hex-pre">{block}</pre>}
        </For>
      </div>
      <ContentEnd data={props.data} total={props.total} />
    </div>
  );
}

const ENCODINGS: { value: TextEncodingChoice; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "utf-8", label: "UTF-8" },
  { value: "utf-16le", label: "UTF-16LE" },
  { value: "utf-16be", label: "UTF-16BE" },
  { value: "windows-1252", label: "Windows-1252" },
];

function TextView(props: { data: Uint8Array; total: number | null }) {
  const [encoding, setEncoding] = createSignal<TextEncodingChoice>("auto");
  const [wrap, setWrap] = createSignal(true);
  const [lineNumbers, setLineNumbers] = createSignal(false);

  const isText = createMemo(() => isProbablyText(props.data));
  const decoded = createMemo(() => decodeText(props.data, encoding()));
  const text = createMemo(() => visibleControls(decoded().text));
  // Only split into lines for the numbered view: one element per line.
  const lines = createMemo(() => (lineNumbers() ? text().split("\n") : []));
  const gutter = () => `${String(lines().length).length}ch`;

  return (
    <>
      <Show when={!isText()}>
        <div class="alert alert-info py-2">
          File appears to be binary; showing it decoded as text anyway.
        </div>
      </Show>
      <div class="card">
        <div class="card-header py-1 d-flex flex-wrap align-items-center gap-3">
          <div class="d-flex align-items-center gap-1">
            <label class="form-label small mb-0" for="file-preview-encoding">
              Encoding
            </label>
            <select
              id="file-preview-encoding"
              class="form-select form-select-sm w-auto"
              value={encoding()}
              onChange={(e) =>
                setEncoding(e.currentTarget.value as TextEncodingChoice)
              }
            >
              <For each={ENCODINGS}>
                {(option) => (
                  <option value={option.value}>{option.label}</option>
                )}
              </For>
            </select>
            <Show when={encoding() === "auto"}>
              <span class="small text-muted">({decoded().encoding})</span>
            </Show>
          </div>
          <div class="form-check small mb-0">
            <input
              id="file-preview-wrap"
              class="form-check-input"
              type="checkbox"
              checked={wrap()}
              onChange={(e) => setWrap(e.currentTarget.checked)}
            />
            <label class="form-check-label" for="file-preview-wrap">
              Wrap
            </label>
          </div>
          <div class="form-check small mb-0">
            <input
              id="file-preview-line-numbers"
              class="form-check-input"
              type="checkbox"
              checked={lineNumbers()}
              onChange={(e) => setLineNumbers(e.currentTarget.checked)}
            />
            <label class="form-check-label" for="file-preview-line-numbers">
              Line numbers
            </label>
          </div>
        </div>
        <pre
          class="card-body app-file-content app-file-text font-monospace small mb-0"
          classList={{ "app-file-text-wrap": wrap() }}
        >
          <Show when={lineNumbers()} fallback={text()}>
            <For each={lines()}>
              {(line, i) => (
                <div class="app-file-line">
                  <span
                    class="app-file-line-number"
                    style={{ width: gutter() }}
                  >
                    {i() + 1}
                  </span>
                  <span class="app-file-line-text">{line}</span>
                </div>
              )}
            </For>
          </Show>
        </pre>
        <ContentEnd data={props.data} total={props.total} />
      </div>
    </>
  );
}

function StringsView(props: { data: Uint8Array; allLoaded: boolean }) {
  const [minLen, setMinLen] = createSignal(6);
  const [filter, setFilter] = createSignal("");
  const [tag, setTag] = createSignal<StringTag | null>(null);

  const extracted = createMemo(() => {
    const result = extractStrings(props.data, 0, minLen());
    return {
      truncated: result.truncated,
      strings: result.strings.map((s) => ({ ...s, tags: classify(s.value) })),
    };
  });

  const tagCounts = createMemo(() => {
    const counts = new Map<StringTag, number>();
    for (const s of extracted().strings) {
      for (const t of s.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    }
    return counts;
  });

  const matching = createMemo(() => {
    const needle = filter().trim().toLowerCase();
    const wanted = tag();
    return extracted().strings.filter(
      (s) =>
        (wanted === null || s.tags.includes(wanted)) &&
        (needle === "" || s.value.toLowerCase().includes(needle)),
    );
  });

  const scope = () =>
    props.allLoaded
      ? `${props.data.length.toLocaleString()} bytes`
      : `first ${props.data.length.toLocaleString()} bytes`;

  return (
    <>
      <div class="d-flex flex-wrap align-items-center gap-2 mb-2">
        <label class="form-label mb-0" for="file-preview-min-len">
          Min length
        </label>
        <select
          id="file-preview-min-len"
          class="form-select form-select-sm w-auto"
          value={String(minLen())}
          onChange={(e) => setMinLen(Number(e.currentTarget.value))}
        >
          <For each={[4, 6, 8]}>
            {(n) => <option value={String(n)}>{n}</option>}
          </For>
        </select>
        <input
          type="search"
          class="form-control form-control-sm w-auto"
          placeholder="Filter"
          value={filter()}
          onInput={(e) => setFilter(e.currentTarget.value)}
        />
        <For each={[...STRING_TAGS]}>
          {(t) => (
            <Show when={tagCounts().has(t)}>
              <button
                type="button"
                class={
                  "btn btn-sm py-0 " +
                  (tag() === t ? "btn-primary" : "btn-outline-secondary")
                }
                onClick={() => setTag(tag() === t ? null : t)}
              >
                {t} ({tagCounts().get(t)})
              </button>
            </Show>
          )}
        </For>
      </div>
      <div class="small text-muted mb-1">
        {extracted().strings.length.toLocaleString()} strings from {scope()}
        <Show when={matching().length !== extracted().strings.length}>
          , {matching().length.toLocaleString()} matching
        </Show>
        <Show when={extracted().truncated}>
          {" "}
          — stopped after {extracted().strings.length.toLocaleString()} strings
        </Show>
        <Show when={matching().length > MAX_STRING_ROWS}>
          {" "}
          — showing the first {MAX_STRING_ROWS.toLocaleString()}; filter to
          narrow
        </Show>
      </div>
      <table class="table table-sm table-striped mb-0 small">
        <thead>
          <tr>
            <th style="width: 1%">Offset</th>
            <th style="width: 1%" title="A: ASCII, W: UTF-16LE">
              Enc
            </th>
            <th>String</th>
            <th style="width: 1%">Tags</th>
          </tr>
        </thead>
        <tbody>
          <For each={matching().slice(0, MAX_STRING_ROWS)}>
            {(s) => (
              <tr>
                <td class="font-monospace">
                  {s.offset.toString(16).padStart(8, "0")}
                </td>
                <td>{s.encoding}</td>
                <td class="font-monospace text-break">
                  {visibleControls(s.value)}
                </td>
                <td class="text-nowrap">{s.tags.join(" ")}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </>
  );
}
