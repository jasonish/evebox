// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Setter } from "solid-js";

export class Logger {
  constructor(private prefix: string, announce: boolean = false) {
    if (announce) {
      this.log("*************************************************");
    }
  }

  log(msg: string) {
    console.log(`${this.prefix}: ${msg}`);
  }
}

// Utility function to wrap a promise with managing a loading counter.
export async function loadingTracker(
  setter: Setter<number>,
  fn: () => Promise<any>
) {
  let delay = 100;
  setter((c) => {
    if (delay > 0) {
      delay = delay * c;
    }
    return c + 1;
  });
  return fn().finally(() => {
    setTimeout(() => {
      setter((c) => c - 1);
    }, delay);
  });
}

export function getVisibleRowCount(offsetId: string) {
  let elOffset = 0;
  let el = document.getElementById(offsetId);
  if (el) {
    elOffset = el.offsetTop;
  }
  console.log(elOffset);

  // Might actually be 57, but with a border, make it 60.
  let rowHeight = 60;

  let windowHeight = window.innerHeight - elOffset - 80;
  let numRows = Math.floor(windowHeight / rowHeight);
  return numRows;
}
