// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import {
  Navigate,
  Route,
  HashRouter,
  useLocation,
  useNavigate,
} from "@solidjs/router";
import { createEffect, createSignal, onMount } from "solid-js";
import { Transition } from "solid-transition-group";

import * as API from "./api";
import { Alerts, AlertState } from "./Alerts";
import { Loader } from "./Loader";
import { Login } from "./Login";
import { Settings } from "./Settings";

import "./styles/transitions.scss";
import { EventView } from "./EventView";
import { Notifications } from "./Notifications";
import { Events } from "./Events";
import { Stats } from "./Stats";
import { serverConfigSet } from "./config";
import { AddressReport } from "./reports/AddressReport";
import { AlertsReport } from "./reports/AlertsReport";
import { OverviewReport } from "./reports/OverviewReport";
import { DhcpReport } from "./reports/DhcpReport";
import { IS_AUTHENTICATED, SET_IS_AUTHENTICATED } from "./global";
import { Ja4Report } from "./pages/ja4";
import { Admin } from "./pages/admin/Admin";
import { AdminFilters } from "./pages/admin/AdminFilters";

export function AppRouter() {
  return (
    <HashRouter root={App}>
      <Route path={"/"} component={AuthenticationRequired}>
        <Route path={"/"} component={RedirectToIndex} />

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
        <Route path={"reports/overview"} component={OverviewReport} />
        <Route path={"reports/alerts"} component={AlertsReport} />
        <Route path={"reports/dhcp"} component={DhcpReport} />
        <Route path={"reports/address/:address"} component={AddressReport} />
        <Route path="ja4/:ja4" component={Ja4Report} />
        <Route path={"stats"} component={Stats} />

        <Route path="admin" component={Admin} />
        <Route path="admin/filters" component={AdminFilters} />

        <Route path="*" component={RedirectToIndex} />
      </Route>
      <Route path={"/login"} component={Login} />
    </HashRouter>
  );
}

function App(props: any) {
  return <>{props.children}</>;
}

function RedirectToIndex() {
  return <Navigate href="/inbox" />;
}

function AuthenticationRequired(props: any) {
  const [loading, setLoading] = createSignal(true);
  const navigate = useNavigate();
  const location = useLocation();
  let mounted = false;

  createEffect(() => {
    if (!IS_AUTHENTICATED() && mounted) {
      console.log(
        "App is mounted but not authenticated, redirecting to login."
      );
      navigate(
        `/login?redirectTo=${encodeURIComponent(
          location.pathname + "?" + location.search
        )}`
      );
    }
  });

  onMount(() => {
    console.log("Wrapper.onMount: checking user");
    API.getUser()
      .then((user: any) => {
        console.log(`Hello ${user.username}`);
        console.log(user);
        if (!user.anonymous) {
          SET_IS_AUTHENTICATED(true);
        }
        API.getConfig().then((config) => {
          console.log("Got server config:");
          console.log(config);
          serverConfigSet(config);
          setLoading(false);
        });
      })
      .catch((error: any) => {
        console.log(`Failed to get user: ${error}`);
        navigate(
          `/login?redirectTo=${encodeURIComponent(
            location.pathname + "?" + location.search
          )}`
        );
      });
    mounted = true;
  });

  return (
    <>
      <Transition name={"fade"}>{loading() && <Loader />}</Transition>
      <Transition name={"fade"}>
        {!loading() && (
          <div>
            <Notifications />
            {props.children}
          </div>
        )}
      </Transition>
    </>
  );
}
