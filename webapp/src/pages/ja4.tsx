// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { useParams } from "@solidjs/router";
import { TIME_RANGE, Top } from "../Top";
import * as api from "../api";
import { createEffect, createResource, createSignal, Show } from "solid-js";
import { CountValueDataTable } from "../components/CountValueDataTable";
import { SearchLink } from "../common/SearchLink";

function getPrefix(ja4: string) {
  if (ja4.startsWith("t")) {
    return "tls";
  } else if (ja4.startsWith("q")) {
    return "quic";
  } else {
    // TODO: Throw an error?
    return "";
  }
}

export function Ja4Report() {
  const params = useParams<{ ja4: string }>();
  const prefix = getPrefix(params.ja4);
  const q = `${prefix}.ja4:${params.ja4}`;
  const [parsed, setParsed] = createSignal<null | Ja4>(null);

  createEffect(() => {
    try {
      const parsed = parseJa4(params.ja4);
      setParsed(parsed);
    } catch (e) {}
  });

  const [ja4db] = createResource(params.ja4, async () => {
    const ja4db_entry = await api.get(`/api/ja4db/${params.ja4}`);
    return ja4db_entry.data;
  });

  const [topSnis] = createResource(TIME_RANGE, async () => {
    let snis = await api.fetchAgg({
      field: `${prefix}.sni`,
      q: q,
      time_range: TIME_RANGE(),
    });
    return snis.rows;
  });

  const [leastSnis] = createResource(TIME_RANGE, async () => {
    let snis = await api.fetchAgg({
      field: `${prefix}.sni`,
      q: q,
      order: "asc",
      time_range: TIME_RANGE(),
    });
    return snis.rows;
  });

  const [topSourceIps] = createResource(TIME_RANGE, async () => {
    let agg = await api.fetchAgg({
      field: `src_ip`,
      q: q,
      time_range: TIME_RANGE(),
    });
    return agg.rows;
  });

  const [leastSourceIps] = createResource(TIME_RANGE, async () => {
    let agg = await api.fetchAgg({
      field: "src_ip",
      q: q,
      order: "asc",
      time_range: TIME_RANGE(),
    });
    return agg.rows;
  });

  const [topDestIps] = createResource(TIME_RANGE, async () => {
    let agg = await api.fetchAgg({
      field: "dest_ip",
      q: q,
      time_range: TIME_RANGE(),
    });
    return agg.rows;
  });

  const [leastDestIps] = createResource(TIME_RANGE, async () => {
    let agg = await api.fetchAgg({
      field: "dest_ip",
      q: q,
      order: "asc",
      time_range: TIME_RANGE(),
    });
    return agg.rows;
  });

  return (
    <>
      <Top />

      <div class="container-fluid mt-2">
        <div class="row">
          <div class="col">
            <h2>
              JA4:
              <SearchLink
                field={prefix + ".ja4"}
                value={params.ja4}
                class="text-decoration-none"
              >
                {" "}
                {params.ja4}
              </SearchLink>
            </h2>
          </div>
        </div>

        <Show when={parsed()}>
          <div class="row mt-1">
            <div class="col">
              <div class="blockquote-footer">
                Protocol: {parsed()!.proto}
                {", "}
                Version: {parsed()!.version}
                {", "}
                SNI: {parsed()!.sni}
                {", "}
                Ciphers: {parsed()!.ciphers}
                {", "}
                Extensions: {parsed()!.extensions}
              </div>
              <Show when={ja4db() && ja4db().user_agent_string}>
                <div class="blockquote-footer">
                  User-Agent: {ja4db().user_agent_string}
                </div>
              </Show>
            </div>
          </div>
        </Show>

        <div class="row">
          <div class="col mb-2">
            <CountValueDataTable
              title="Top SNIs"
              label="SNI"
              rows={topSnis() || []}
              loading={topSnis.loading}
            />
          </div>
          <div class="col mb-2">
            <CountValueDataTable
              title="Least SNIs"
              label="SNI"
              rows={leastSnis() || []}
              loading={leastSnis.loading}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mb-2">
            <CountValueDataTable
              title="Top Source IPs"
              label="IP"
              rows={topSourceIps() || []}
              loading={topSourceIps.loading}
            />
          </div>
          <div class="col mb-2">
            <CountValueDataTable
              title="Least Source IPs"
              label="IP"
              rows={leastSourceIps() || []}
              loading={leastSourceIps.loading}
            />
          </div>
        </div>

        <div class="row">
          <div class="col mb-2">
            <CountValueDataTable
              title="Top Destination IPs"
              label="IP"
              rows={topDestIps() || []}
              loading={topDestIps.loading}
            />
          </div>
          <div class="col mb-2">
            <CountValueDataTable
              title="Least Destination IPs"
              label="IP"
              rows={leastDestIps() || []}
              loading={leastDestIps.loading}
            />
          </div>
        </div>
      </div>
    </>
  );
}

interface Ja4 {
  proto: string;
  version: string;
  sni: string;
  ciphers: number;
  extensions: number;
  alpn: string;
}

function parseJa4(ja4: string): null | Ja4 {
  const matches = ja4.match(/(.)(..)(.)(..)(..)(..)/);

  if (matches) {
    const proto = ((s) => {
      switch (s) {
        case "t":
          return "tls";
        case "q":
          return "quic";
        default:
          return s;
      }
    })(matches[1]);

    const version = ((s) => {
      return `${s[0]}.${s[1]}`;
    })(matches[2]);

    const sni = ((s) => {
      switch (s) {
        case "d":
          return "domain";
        case "i":
          return "ip";
        default:
          return s;
      }
    })(matches[3]);

    let parsed = {
      proto: proto,
      version: version,
      sni: sni,
      ciphers: parseInt(matches[4]),
      extensions: parseInt(matches[5]),
      alpn: matches[6],
    };
    return parsed;
  }
  return null;
}
