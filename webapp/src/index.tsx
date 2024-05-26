// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { render } from "solid-js/web";

import { loadInitialTheme } from "./settings";
import { AppRouter } from "./App";

import "./styles/index.scss";

// Initialize Chartjs.
import { Chart, registerables } from "chart.js";
import "chartjs-adapter-dayjs-4";

Chart.register(...registerables);

loadInitialTheme();

const root = document.getElementById("root");
render(() => <AppRouter />, root!);
