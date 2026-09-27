// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Download of files extracted by Suricata's file-store output.

import { createSignal, For, Show } from "solid-js";
import { Button, Dropdown, Spinner } from "solid-bootstrap";
import { API } from "./api";
import { addError } from "./Notifications";
import { EventSource } from "./types";

// A file referenced by an event that may be in a file store.
export interface EventFile {
  sha256: string;
  filename?: string;
  size?: number;
}

const SHA256_RE = /^[0-9a-fA-F]{64}$/;

// The files an event references, from its fileinfo object and an
// alert's files array, deduplicated by digest. The file store is
// content addressed, so a file whose own occurrence was not stored may
// still be available; the server decides.
export function eventFiles(source: EventSource | undefined): EventFile[] {
  if (!source) return [];
  const files: EventFile[] = [];
  const add = (entry: any) => {
    const sha256 = entry?.sha256;
    if (typeof sha256 !== "string" || !SHA256_RE.test(sha256)) return;
    const lower = sha256.toLowerCase();
    if (files.some((file) => file.sha256 === lower)) return;
    files.push({
      sha256: lower,
      filename: typeof entry.filename === "string" ? entry.filename : undefined,
      size: typeof entry.size === "number" ? entry.size : undefined,
    });
  };
  add(source.fileinfo);
  const list = (source as any).files;
  if (Array.isArray(list)) {
    list.forEach(add);
  }
  return files;
}

export function fileErrorMessage(err: API.FileDownloadError): string {
  switch (err.code) {
    case "file-not-found":
      return "File is not in the file store (it was not stored, or has been pruned).";
    case "no-source":
      return `No file source available: ${err.message}`;
    case "ambiguous-source":
      return "More than one file source could serve this file.";
    default:
      return err.message;
  }
}

function describe(file: EventFile): string {
  const name = file.filename || file.sha256.slice(0, 16);
  return file.size !== undefined ? `${name} (${file.size} bytes)` : name;
}

// Validate, then hand the transfer to the browser. Errors become toasts.
export async function downloadFile(
  eventId: string | undefined,
  file: EventFile,
) {
  const params: API.FileRequestParams = { eventId, sha256: file.sha256 };
  try {
    await API.validateFile(params);
    API.startFileDownload(params, (err) => addError(fileErrorMessage(err)));
  } catch (err) {
    if (err instanceof API.FileDownloadError) {
      addError(fileErrorMessage(err));
    } else {
      addError(`File request failed: ${err}`);
    }
  }
}

// Event view card-header action: a button for a single file, or a menu
// when the event references several.
export function FileDownloadButton(props: {
  eventId: string;
  files: EventFile[];
  style?: string;
}) {
  const [pending, setPending] = createSignal(false);

  const download = async (file: EventFile) => {
    setPending(true);
    try {
      await downloadFile(props.eventId, file);
    } finally {
      setPending(false);
    }
  };

  const label = () => (
    <Show when={pending()} fallback={"Download"}>
      <Spinner
        as="span"
        animation="border"
        size="sm"
        role="status"
        aria-hidden="true"
      />{" "}
      Download
    </Show>
  );

  return (
    <Show
      when={props.files.length > 1}
      fallback={
        <Button
          style={props.style}
          disabled={pending()}
          title={`${props.files[0]?.filename ?? ""}\nSHA256: ${props.files[0]?.sha256}`}
          onclick={() => download(props.files[0])}
        >
          {label()}
        </Button>
      }
    >
      <Dropdown class={"d-inline-block"} align={"end"}>
        <Dropdown.Toggle style={props.style} disabled={pending()}>
          {label()}
        </Dropdown.Toggle>
        <Dropdown.Menu>
          <For each={props.files}>
            {(file) => (
              <Dropdown.Item
                title={`SHA256: ${file.sha256}`}
                onClick={() => download(file)}
              >
                {describe(file)}
              </Dropdown.Item>
            )}
          </For>
        </Dropdown.Menu>
      </Dropdown>
    </Show>
  );
}
