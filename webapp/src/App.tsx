// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import {
  Navigate,
  Outlet,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from "@solidjs/router";
import type { Component } from "solid-js";
import { createSignal, onMount } from "solid-js";
import { Transition } from "solid-transition-group";

import * as API from "./api";
import { Alerts, AlertState } from "./Alerts";
import { Loader } from "./Loader";
import { Top } from "./Top";
import { Login } from "./Login";
import { Settings } from "./Settings";

import "./transitions.scss";
import { EventView } from "./EventView";
import { Notifications } from "./Notifications";
import { Events } from "./Events";
import { Alert, Col, Container, Row } from "solid-bootstrap";
import { Stats } from "./Stats";
import { serverConfigSet } from "./config";
import { AddressReport } from "./reports/AddressReport";
import { AlertsReport } from "./reports/AlertsReport";
import { OverviewReport } from "./reports/OverviewReport";

const Report: Component = () => {
  return (
    <div>
      <Top />
      <Container>
        <Row>
          <Col class={"mt-2"}>
            <Alert>
              Sorry, the web part of EveBox is being rewritten and reports will
              be getting an overhaul, but reports are not yet ready.
            </Alert>
          </Col>
        </Row>
      </Container>
    </div>
  );
};

export function App() {
  return (
    <Routes>
      <Route path={"/"} component={Wrapper}>
        <Route path={"/"} element={<Navigate href={"/inbox"} />} />

        <Route path={"inbox"} component={AlertState}>
          <Route path={"/"} component={Alerts} />
          <Route path={":id"} component={EventView} />
        </Route>

        <Route path={"alerts"} component={AlertState}>
          <Route path={"/"} component={Alerts} />
          <Route path={":id"} component={EventView} />
        </Route>

        <Route path={"escalated"} component={AlertState}>
          <Route path={"/"} component={Alerts} />
          <Route path={":id"} component={EventView} />
        </Route>

        <Route path={"events"} component={Events} />
        <Route path={"event/:id"} component={EventView} />
        <Route path={"settings"} component={Settings} />
        <Route path={"reports"} component={Report} />
        <Route path={"reports/address/:address"} component={AddressReport} />
        <Route path={"reports/alerts"} component={AlertsReport} />
        <Route path={"reports/overview"} component={OverviewReport} />
        <Route path={"stats"} component={Stats} />
        <Route path={"*"} element={<Navigate href={"/inbox"} />} />
      </Route>
      <Route path={"/login"} component={Login} />
    </Routes>
  );
}

function Wrapper() {
  const [loading, setLoading] = createSignal(true);
  const navigate = useNavigate();
  const location = useLocation();

  onMount(async () => {
    console.log("Wrapper.onMount: checking user");
    API.getUser()
      .then((user) => {
        console.log(`Hello ${user.username}`);
        API.getConfig().then((config) => {
          serverConfigSet(config);
          setLoading(false);
        });
      })
      .catch((error) => {
        console.log(`Failed to get user: ${error}`);
        navigate(
          `/login?redirectTo=${encodeURIComponent(
            location.pathname + "?" + location.search
          )}`
        );
      });
  });

  return (
    <>
      <Transition name={"fade"}>{loading() && <Loader />}</Transition>
      <Transition name={"fade"}>
        {!loading() && (
          <div>
            <Notifications />
            <Outlet />
          </div>
        )}
      </Transition>
    </>
  );
}
