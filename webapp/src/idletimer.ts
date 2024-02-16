// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { createSignal } from "solid-js";

export class IdleTimer {
  private intervalId: null | number = null;
  public timeout: () => number;
  private setTimeout: any = null;

  private eventListener = () => {
    this.resetTimer();
  };

  constructor(private ms: number = 60000) {
    [this.timeout, this.setTimeout] = createSignal(0);
    window.addEventListener("mousemove", this.eventListener);
    window.addEventListener("keypress", this.eventListener);
    this.resetTimer();
  }

  private resetTimer() {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
    }
    this.intervalId = setInterval(() => {
      console.log("IdleTimer: TIMEOUT");
      this.setTimeout((n: number) => n + 1);
    }, this.ms);
  }

  public stop() {
    window.removeEventListener("keypress", this.eventListener);
    window.removeEventListener("mousemove", this.eventListener);
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
    }
  }
}
