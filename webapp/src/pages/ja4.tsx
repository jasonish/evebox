// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { useParams } from "@solidjs/router";
import { TIME_RANGE, Top } from "../Top";
import * as api from "../api";
import { createResource } from "solid-js";
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
              <SearchLink field={prefix + ".ja4"} value={params.ja4}>
                {params.ja4}
              </SearchLink>
            </h2>
          </div>
        </div>

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
