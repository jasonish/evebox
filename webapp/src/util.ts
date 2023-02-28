// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

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
