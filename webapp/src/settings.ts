// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import dark from "./styles/dark.scss";
import light from "./styles/light.scss";
import { createSignal } from "solid-js";
import { TIME_RANGE } from "./Top";
import { parse_timerange } from "./datetime";

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
      e.innerHTML = dark;
      name = "dark";
      break;
    default:
      e.innerHTML = light;
      name = "light";
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
