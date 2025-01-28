// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { TIME_RANGE, Top } from "../Top";
import { useParams } from "@solidjs/router";
import { createEffect, createSignal, For, onCleanup } from "solid-js";
import { Col, Container, Row } from "solid-bootstrap";
import { API, AggRequest, AggResponseRow } from "../api";
import { CountValueDataTable } from "../components";
import { SetStoreFunction, createStore } from "solid-js/store";
import { RefreshButton } from "../common/RefreshButton";
import dayjs from "dayjs";

interface AggResults {
  loading: boolean;
  rows: AggResponseRow[];
  timestamp: null | dayjs.Dayjs;
}

function defaultAggResults(): AggResults {
  return {
    loading: false,
    rows: [],
    timestamp: null,
  };
}

export function Address() {
  const params = useParams<{ address: string }>();

  const [loading, setLoading] = createSignal(0);

  // For SSE cancellation.
  const [version, setVersion] = createSignal(0);

  const [mostAlertingSignature, setMostAlertingSignature] = createStore(
    defaultAggResults()
  );

  const [leastAlertingSignature, setLeastAlertingSignature] = createStore(
    defaultAggResults()
  );

  const [mostRequestedDns, setMostRequestedDns] = createStore(
    defaultAggResults()
  );

  const [leastRequestedDns, setLeastRequestedDns] = createStore(
    defaultAggResults()
  );

  const [mostHttpUserAgents, setMostHttpUserAgents] = createStore(
    defaultAggResults()
  );

  const [leastHttpUserAgents, setLeastHttpUserAgents] = createStore(
    defaultAggResults()
  );

  const [mostRequestedTlsSni, setMostRequestedTlsSni] = createStore(
    defaultAggResults()
  );

  const [leastRequestedTlsSni, setLeastRequestedTlsSni] = createStore(
    defaultAggResults()
  );

  const [mostSshClientVersions, setMostSshClientVersions] = createStore(
    defaultAggResults()
  );

  const [leastSshClientVersions, setLeastSshClientVersions] = createStore(
    defaultAggResults()
  );

  const [mostSshServerVersions, setMostSshServerVersions] = createStore(
    defaultAggResults()
  );

  const [leastSshServerVersions, setLeastSshServerVersions] = createStore(
    defaultAggResults()
  );

  const [httpTopOutboundHostnames, setHttpTopOutboundHostnames] = createStore(
    defaultAggResults()
  );

  const [httpLeastOutboundHostnames, setHttpLeastOutboundHostnames] =
    createStore(defaultAggResults());

  const [httpTopInboundHostnames, setHttpTopInboundHostnames] = createStore(
    defaultAggResults()
  );

  const [httpLeastInboundHostnames, setHttpLeastInboundHostnames] = createStore(
    defaultAggResults()
  );

  const [tlsSniInboundTop, setTlsSniInboundTop] = createStore(
    defaultAggResults()
  );

  const [tlsSniInboundLeast, setTlsSniInboundLeast] = createStore(
    defaultAggResults()
  );

  const [tlsMostRequestedSubjects, setTlsMostRequestedSubjects] = createStore(
    defaultAggResults()
  );

  const [tlsLeastRequestedSubjects, setTlsLeastRequestedSubjects] = createStore(
    defaultAggResults()
  );

  const [tlsMostRequestedIssueDn, setTlsMostRequestedIssueDn] = createStore(
    defaultAggResults()
  );

  const [tlsLeastRequestedIssueDn, setTlsLeastRequestedIssueDn] = createStore(
    defaultAggResults()
  );

  onCleanup(() => {
    API.cancelAllSse();
  });

  createEffect(() => {
    forceRefresh();
  });

  function forceRefresh() {
    refresh(TIME_RANGE());
  }

  // NOTE: We can't use ${params.address} here as this structure is static and not subject
  //   to reactive updates.  Thre refresh function will substitute {{address}} with the current
  //   address when it changes.
  const LOADERS: {
    request: AggRequest;
    setter: SetStoreFunction<AggResults>;
    getter: AggResults;
    title: string;
    label: string;
  }[] = [
    // Most alerting rules.
    {
      request: {
        field: "alert.signature",
        order: "desc",
        q: `event_type:alert @ip:{{address}}`,
        size: 10,
      },
      setter: setMostAlertingSignature,
      getter: mostAlertingSignature,
      title: "Most Alerting Rules",
      label: "Signature",
    },

    // Least alerting rules.
    {
      request: {
        field: "alert.signature",
        order: "asc",
        q: `event_type:alert @ip:{{address}}`,
        size: 10,
      },
      setter: setLeastAlertingSignature,
      getter: leastAlertingSignature,
      title: "Least Alerting Rules",
      label: "Signature",
    },

    // Most requested DNS hostnames.
    {
      request: {
        field: "dns.rrname",
        order: "desc",
        q: `event_type:dns dns.type:query src_ip:{{address}}`,
        size: 10,
      },
      setter: setMostRequestedDns,
      getter: mostRequestedDns,
      title: "Most Requested DNS Hostnames",
      label: "Hostname",
    },

    // Least requested DNS hostnames.
    {
      request: {
        field: "dns.rrname",
        order: "asc",
        q: `event_type:dns dns.type:query src_ip:{{address}}`,
        size: 10,
      },
      setter: setLeastRequestedDns,
      getter: leastRequestedDns,
      title: "Least Requested DNS Hostnames",
      label: "Hostname",
    },

    // Top outbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "desc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: setHttpTopOutboundHostnames,
      getter: httpTopOutboundHostnames,
      title: "Top Outbound HTTP Hostnames",
      label: "Hostname",
    },

    // Least outbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "asc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: setHttpLeastOutboundHostnames,
      getter: httpLeastOutboundHostnames,
      title: "Least Outbound HTTP Hostnames",
      label: "Hostname",
    },

    // Top inbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "desc",
        q: `event_type:http dest_ip:{{address}}`,
        size: 10,
      },
      setter: setHttpTopInboundHostnames,
      getter: httpTopInboundHostnames,
      title: "Top Inbound HTTP Hostnames",
      label: "Hostname",
    },

    // Least inbound HTTP hostnames
    {
      request: {
        field: "http.hostname",
        order: "asc",
        q: `event_type:http dest_ip:{{address}}`,
        size: 10,
      },
      setter: setHttpLeastInboundHostnames,
      getter: httpLeastInboundHostnames,
      title: "Least Inbound HTTP Hostnames",
      label: "Hostname",
    },

    // Most HTTP user agents.
    {
      request: {
        field: "http.http_user_agent",
        order: "desc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: setMostHttpUserAgents,
      getter: mostHttpUserAgents,
      title: "Top Outbound HTTP User Agents",
      label: "User Agent",
    },

    // Least HTTP user agents.
    {
      request: {
        field: "http.http_user_agent",
        order: "asc",
        q: `event_type:http src_ip:{{address}}`,
        size: 10,
      },
      setter: setLeastHttpUserAgents,
      getter: leastHttpUserAgents,
      title: "Least Outbound HTTP User Agents",
      label: "User Agent",
    },

    // Most TLS SNI.
    {
      request: {
        field: "tls.sni",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setMostRequestedTlsSni,
      getter: mostRequestedTlsSni,
      title: "Most Requested TLS SNI Names",
      label: "Name",
    },

    // Least TLS SNI.
    {
      request: {
        field: "tls.sni",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setLeastRequestedTlsSni,
      getter: leastRequestedTlsSni,
      title: "Least Requested TLS SNI Names",
      label: "Name",
    },

    // TLS: Top Inbound SNI
    {
      request: {
        field: "tls.sni",
        order: "desc",
        q: `event_type:tls dest_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsSniInboundTop,
      getter: tlsSniInboundTop,
      title: "Top Inbound TLS SNI Names",
      label: "Name",
    },

    // TLS: Least Inbound SNI
    {
      request: {
        field: "tls.sni",
        order: "asc",
        q: `event_type:tls dest_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsSniInboundLeast,
      getter: tlsSniInboundLeast,
      title: "Least Inbound TLS SNI Names",
      label: "Name",
    },

    // Top Requests TLS Subjects
    {
      request: {
        field: "tls.subject",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsMostRequestedSubjects,
      getter: tlsMostRequestedSubjects,
      title: "Most Requested TLS Subjects",
      label: "Name",
    },

    // Least Requested TLS Subjects
    {
      request: {
        field: "tls.subject",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsLeastRequestedSubjects,
      getter: tlsLeastRequestedSubjects,
      title: "Least Requested TLS Subjects",
      label: "Name",
    },

    // Top Requested TLS Issuer DN
    {
      request: {
        field: "tls.issuerdn",
        order: "desc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsMostRequestedIssueDn,
      getter: tlsMostRequestedIssueDn,
      title: "Most Requested TLS Issuer DN",
      label: "Name",
    },

    // Least Requested TLS Issuer DN
    {
      request: {
        field: "tls.issuerdn",
        order: "asc",
        q: `event_type:tls src_ip:{{address}}`,
        size: 10,
      },
      setter: setTlsLeastRequestedIssueDn,
      getter: tlsLeastRequestedIssueDn,
      title: "Least Requested TLS Issuer DN",
      label: "Name",
    },

    // Most SSH client versions
    {
      request: {
        field: "ssh.client.software_version",
        order: "desc",
        q: `event_type:ssh src_ip:{{address}}`,
        size: 10,
      },
      setter: setMostSshClientVersions,
      getter: mostSshClientVersions,
      title: "Most SSH Client Software Versions",
      label: "Version",
    },

    // Least SSH client versions
    {
      request: {
        field: "ssh.client.software_version",
        order: "asc",
        q: `event_type:ssh src_ip:{{address}}`,
        size: 10,
      },
      setter: setLeastSshClientVersions,
      getter: leastSshClientVersions,
      title: "Least SSH Client Software Versions",
      label: "Version",
    },

    // Most SSH server versions
    {
      request: {
        field: "ssh.server.software_version",
        order: "desc",
        q: `event_type:ssh dest_ip:{{address}}`,
        size: 10,
      },
      setter: setMostSshServerVersions,
      getter: mostSshServerVersions,
      title: "Most SSH Server Software Versions",
      label: "Version",
    },

    // Least SSH server versions
    {
      request: {
        field: "ssh.server.software_version",
        order: "asc",
        q: `event_type:ssh dest_ip:{{address}}`,
        size: 10,
      },
      setter: setLeastSshServerVersions,
      getter: leastSshServerVersions,
      title: "Least SSH Server Software Versions",
      label: "Version",
    },
  ];

  function refresh(timeRange: string) {
    setVersion((version) => version + 1);
    setLoading(LOADERS.length);

    for (const loader of LOADERS) {
      let request = { time_range: timeRange, ...loader.request };
      request.q = request.q?.replace("{{address}}", params.address);

      loader.setter("loading", true);

      API.getSseAgg(request, version, (data: any) => {
        if (data) {
          loader.setter("timestamp", dayjs(data.earliest_ts));
          loader.setter("rows", data.rows);
        }
      }).finally(() => {
        loader.setter("loading", false);
        setLoading((n) => n - 1);
      });
    }
  }

  const formatSuffix = (timestamp: dayjs.Dayjs | null) => {
    if (timestamp) {
      return `since ${timestamp.fromNow()}`;
    }
    return undefined;
  };

  return (
    <>
      <Top />
      <Container fluid>
        <Row class={"mt-2"}>
          <Col>
            <RefreshButton loading={loading()} refresh={forceRefresh} />
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
                  <CountValueDataTable
                    title={loader.title!}
                    label={loader.label!}
                    rows={loader.getter.rows}
                    loading={loader.getter.loading}
                    suffix={formatSuffix(loader.getter.timestamp)}
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
