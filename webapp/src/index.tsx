// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { render } from "solid-js/web";
import { hashIntegration, Router } from "@solidjs/router";

import { loadInitialTheme } from "./settings";
import { App } from "./App";

import "./styles/index.scss";

// Initialize Chartjs.
import { Chart, registerables, Colors } from "chart.js";
import "chartjs-adapter-dayjs-4";

Chart.register(...registerables);
Chart.register(Colors);

loadInitialTheme();

render(
  () => (
    <Router source={hashIntegration()}>
      <App />
    </Router>
  ),
  document.getElementById("root") as HTMLElement
);
