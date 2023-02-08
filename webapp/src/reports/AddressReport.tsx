// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { useParams } from "@solidjs/router";
import { createEffect, createSignal, For, Show } from "solid-js";
import * as API from "../api";
import { Button, Col, Container, Row } from "solid-bootstrap";
import { GroupByQueryRequest, GroupByQueryResponseRow } from "../api";
import { CountValueTable } from "./AlertsReport";
import { createStore } from "solid-js/store";
import { RefreshButton } from "../common/RefreshButton";

export function AddressReport() {
  const params = useParams<{ address: string }>();
  const [mostSignatures, setMostSignatures] = createSignal<
    GroupByQueryResponseRow[]
  >([]);
  const [leastSignatures, setLeastSignatures] = createSignal<
    GroupByQueryResponseRow[]
  >([]);
  const [loading, setLoading] = createSignal(0);
  const [isLoading, setIsLoading] = createSignal(false);

  const [results, setResults] = createStore<{
    mostRequestedDns: GroupByQueryResponseRow[];
    leastRequestedDns: GroupByQueryResponseRow[];
    mostHttpUserAgents: GroupByQueryResponseRow[];
    leastHttpUserAgents: GroupByQueryResponseRow[];
    mostRequestedTlsSni: GroupByQueryResponseRow[];
    leastRequestedTlsSni: GroupByQueryResponseRow[];
    mostSshClientVersions: GroupByQueryResponseRow[];
    leastSshClientVersions: GroupByQueryResponseRow[];
    mostSshServerVersions: GroupByQueryResponseRow[];
    leastSshServerVersions: GroupByQueryResponseRow[];

    httpTopOutboundHostnames: GroupByQueryResponseRow[];
    httpLeastOutboundHostnames: GroupByQueryResponseRow[];

    httpTopInboundHostnames: GroupByQueryResponseRow[];
    httpLeastInboundHostnames: GroupByQueryResponseRow[];

    tlsSniInboundTop: GroupByQueryResponseRow[];
    tlsSniInboundLeast: GroupByQueryResponseRow[];

    tlsMostRequestedSubjects: GroupByQueryResponseRow[];
    tlsLeastRequestedSubjects: GroupByQueryResponseRow[];

    tlsMostRequestedIssueDn: GroupByQueryResponseRow[];
    tlsLeastRequestedIssueDn: GroupByQueryResponseRow[];
  }>({
    mostRequestedDns: [],
    leastRequestedDns: [],
    mostHttpUserAgents: [],
    leastHttpUserAgents: [],
    mostRequestedTlsSni: [],
    leastRequestedTlsSni: [],
    mostSshClientVersions: [],
    leastSshClientVersions: [],
    mostSshServerVersions: [],
    leastSshServerVersions: [],

    httpTopOutboundHostnames: [],
    httpLeastOutboundHostnames: [],

    httpTopInboundHostnames: [],
    httpLeastInboundHostnames: [],

    tlsSniInboundTop: [],
    tlsSniInboundLeast: [],

    tlsMostRequestedSubjects: [],
    tlsLeastRequestedSubjects: [],

    tlsMostRequestedIssueDn: [],
    tlsLeastRequestedIssueDn: [],
  });

  createEffect(() => {
    forceRefresh();
  });

  createEffect(() => {
    setIsLoading(loading() > 1);
  });

  function forceRefresh() {
    refresh(TIME_RANGE());
  }

  // NOTE: We can't use ${params.address} here as this structure is static and not subject
  //   to reactive updates.  Thre refresh function will substitute {{address}} with the current
  //   address when it changes.
  const LOADERS: {
    request: GroupByQueryRequest;
    setter: (rows: GroupByQueryResponseRow[]) => void;
    title: string;
    label: string;
    get: () => GroupByQueryResponseRow[];
  }[] = [
    // Most alerting rules.
    {
      request: {
        field: "alert.signature",
        order: "desc",
        q: `event_type:alert @ip:{{address}}`,
        size: 10,
      },
      setter: setMostSignatures,
      title: "Most Alerting Rules",
      label: "Signature",
      get: () => mostSignatures(),
    },

    // Least alerting rules.
    {
      request: {
        field: "alert.signature",
        order: "asc",
        q: `event_type:alert @ip:{{address}}`,
        size: 10,
      },
      setter: setLeastSignatures,
      title: "Least Alerting Rules",
      label: "Signature",
      get: () => leastSignatures(),
    },

    // Most requested DNS hostnames.
    {
      request: {
        field: "dns.rrname",
        order: "desc",
        q: `event_type:dns dns.type:query src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("mostRequestedDns", rows),
      title: "Most Requested DNS Hostnames",
      label: "Hostname",
      get: () => results.mostRequestedDns,
    },

    // Least requested DNS hostnames.
    {
      request: {
        field: "dns.rrname",
        order: "asc",
        q: `event_type:dns dns.type:query src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("leastRequestedDns", rows),
      title: "Least Requested DNS Hostnames",
      label: "Hostname",
      get: () => results.leastRequestedDns,
    },

    // Top outbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "desc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("httpTopOutboundHostnames", rows),
      title: "Top Outbound HTTP Hostnames",
      label: "Hostname",
      get: () => results.httpTopOutboundHostnames,
    },

    // Least outbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "asc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("httpLeastOutboundHostnames", rows),
      title: "Least Outbound HTTP Hostnames",
      label: "Hostname",
      get: () => results.httpLeastOutboundHostnames,
    },

    // Top inbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "desc",
        q: `event_type:http dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("httpTopInboundHostnames", rows),
      title: "Top Inbound HTTP Hostnames",
      label: "Hostname",
      get: () => results.httpTopInboundHostnames,
    },

    // Least inbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "asc",
        q: `event_type:http dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("httpLeastInboundHostnames", rows),
      title: "Least Inbound HTTP Hostnames",
      label: "Hostname",
      get: () => results.httpLeastInboundHostnames,
    },

    // Most HTTP user agents.
    {
      request: {
        field: "http.http_user_agent",
        order: "desc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("mostHttpUserAgents", rows),
      title: "Top Outbound HTTP User Agents",
      label: "User Agent",
      get: () => results.mostHttpUserAgents,
    },

    // Least HTTP user agents.
    {
      request: {
        field: "http.http_user_agent",
        order: "asc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("leastHttpUserAgents", rows),
      title: "Least Outbound HTTP User Agents",
      label: "User Agent",
      get: () => results.leastHttpUserAgents,
    },

    // Most TLS SNI.
    {
      request: {
        field: "tls.sni",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("mostRequestedTlsSni", rows),
      title: "Most Requested TLS SNI Names",
      label: "Name",
      get: () => results.mostRequestedTlsSni,
    },

    // Least TLS SNI.
    {
      request: {
        field: "tls.sni",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("leastRequestedTlsSni", rows),
      title: "Least Requested TLS SNI Names",
      label: "Name",
      get: () => results.leastRequestedTlsSni,
    },

    // TLS: Top Inbound SNI
    {
      request: {
        field: "tls.sni",
        order: "desc",
        q: `event_type:tls dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsSniInboundTop", rows),
      title: "Top Inbound TLS SNI Names",
      label: "Name",
      get: () => results.tlsSniInboundTop,
    },

    // TLS: Least Inbound SNI
    {
      request: {
        field: "tls.sni",
        order: "asc",
        q: `event_type:tls dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsSniInboundLeast", rows),
      title: "Least Inbound TLS SNI Names",
      label: "Name",
      get: () => results.tlsSniInboundLeast,
    },

    // Top Requests TLS Subjects
    {
      request: {
        field: "tls.subject",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsMostRequestedSubjects", rows),
      title: "Most Requested TLS Subjects",
      label: "Name",
      get: () => results.tlsMostRequestedSubjects,
    },

    // Least Requested TLS Subjects
    {
      request: {
        field: "tls.subject",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsLeastRequestedSubjects", rows),
      title: "Least Requested TLS Subjects",
      label: "Name",
      get: () => results.tlsLeastRequestedSubjects,
    },

    // Top Requested TLS Issuer DN
    {
      request: {
        field: "tls.issuerdn",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsMostRequestedIssueDn", rows),
      title: "Most Requested TLS Issuer DN",
      label: "Name",
      get: () => results.tlsMostRequestedIssueDn,
    },

    // Least Requested TLS Issuer DN
    {
      request: {
        field: "tls.issuerdn",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows: GroupByQueryResponseRow[]) =>
        setResults("tlsLeastRequestedIssueDn", rows),
      title: "Least Requested TLS Issuer DN",
      label: "Name",
      get: () => results.tlsLeastRequestedIssueDn,
    },

    // Most SSH client versions
    {
      request: {
        field: "ssh.client.software_version",
        order: "desc",
        q: `event_type:ssh src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows) => setResults("mostSshClientVersions", rows),
      title: "Most SSH Client Software Versions",
      label: "Version",
      get: () => results.mostSshClientVersions,
    },

    // Least SSH client versions
    {
      request: {
        field: "ssh.client.software_version",
        order: "asc",
        q: `event_type:ssh src_ip:{{address}}`,
        size: 10,
      },
      setter: (rows) => setResults("leastSshClientVersions", rows),
      title: "Least SSH Client Software Versions",
      label: "Version",
      get: () => results.leastSshClientVersions,
    },

    // Most SSH server versions
    {
      request: {
        field: "ssh.server.software_version",
        order: "desc",
        q: `event_type:ssh dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows) => setResults("mostSshServerVersions", rows),
      title: "Most SSH Server Software Versions",
      label: "Version",
      get: () => results.mostSshServerVersions,
    },

    // Least SSH server versions
    {
      request: {
        field: "ssh.server.software_version",
        order: "asc",
        q: `event_type:ssh dest_ip:{{address}}`,
        size: 10,
      },
      setter: (rows) => setResults("leastSshServerVersions", rows),
      title: "Least SSH Server Software Versions",
      label: "Version",
      get: () => results.leastSshServerVersions,
    },
  ];

  function refresh(timeRange: string) {
    const runLoader = (i: number) => {
      let loader = LOADERS[i];
      if (!loader) {
        return;
      }
      let request = { time_range: timeRange, ...loader.request };
      request.q = request.q?.replace("{{address}}", params.address);
      API.groupBy(request)
        .then((response) => {
          loader.setter(response.rows);
        })
        .finally(() => {
          runLoader(i + 1);
          setLoading((n) => n - 1);
        });
    };
    setLoading(LOADERS.length);
    runLoader(0);
  }

  return (
    <>
      <Top />
      <Container fluid>
        <Row>
          <Col>
            <RefreshButton
              loading={loading()}
              refresh={forceRefresh}
              showProgress={true}
            />
            <h2 class={"d-inline align-middle ms-2"}>
              Report for {params.address}
            </h2>
          </Col>
        </Row>

        <Row>
          <For each={LOADERS}>
            {(loader) => (
              <>
                <Col class={"mt-2"} md={6}>
                  <CountValueTable
                    title={loader.title!}
                    label={loader.label!}
                    rows={loader.get()}
                  />
                </Col>
              </>
            )}
          </For>
        </Row>
      </Container>
    </>
  );
}
