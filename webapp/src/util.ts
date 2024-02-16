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
export function trackLoading(setter: Setter<number>, fn: () => Promise<any>) {
  // Adding a delay is useful for debugging.
  let delay = 100;
  setter((c) => {
    if (delay > 0) {
      delay = delay * c;
    }
    return c + 1;
  });
  fn().finally(() => {
    setTimeout(() => {
      setter((c) => c - 1);
    }, delay);
  });
}
