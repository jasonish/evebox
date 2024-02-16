// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createSignal } from "solid-js";
import { TIME_RANGE } from "./Top";
import { parse_timerange } from "./datetime";
import { Chart } from "chart.js";

const DEFAULT_THEME = "light";
const DEFAULT_VIEW_SIZE = 100;

export const [currentThemeName, setCurrentThemeName] =
  createSignal(DEFAULT_THEME);

const localViewSize =
  +(localStorage.getItem("VIEW_SIZE") || DEFAULT_VIEW_SIZE) ||
  DEFAULT_VIEW_SIZE;
export const [VIEW_SIZE, SET_VIEW_SIZE] = createSignal(localViewSize);

export function setTheme(name: string) {
  document.getElementById("theme")?.remove();
  let e = document.createElement("style");
  e.id = "theme";
  switch (name) {
    case "dark":
      document.querySelector("html")?.setAttribute("data-bs-theme", "dark");
      Chart.defaults.color = "#fff";
      name = "dark";
      break;
    default:
      name = "light";
      Chart.defaults.color = "#666";
      document.querySelector("html")?.setAttribute("data-bs-theme", "light");
      break;
  }
  document.body.appendChild(e);
  localStorage.setItem("THEME", name);
  setCurrentThemeName(name);
}

export function loadInitialTheme() {
  const localTheme = localStorage.getItem("THEME");
  switch (localTheme) {
    case "dark":
      setTheme("dark");
      break;
    default:
      setTheme("light");
      break;
  }
}

export function setViewSize(size: number) {
  localStorage.setItem("VIEW_SIZE", `${size}`);
  SET_VIEW_SIZE(size);
}

export function timeRangeAsSeconds(): number | undefined {
  let time_range = TIME_RANGE();
  if (time_range !== undefined) {
    return parse_timerange(time_range);
  }
  return undefined;
}
