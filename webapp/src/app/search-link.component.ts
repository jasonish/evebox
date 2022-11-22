/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

import { Component, Input, OnInit } from "@angular/core";

@Component({
  selector: "search-link",
  template: `<a
    [routerLink]="[route]"
    [queryParams]="{ q: queryString }"
    style="word-break: break-all; text-decoration: none;"
    >{{ value }}</a
  >`,
})
export class EveboxSearchLinkComponent implements OnInit {
  @Input() field: string;
  @Input() value: string;
  @Input() searchParams: any;
  @Input() route = "/events";
  @Input() search: string;

  queryString: string;

  ngOnInit() {
    let queryString = "";

    if (!this.search) {
      this.search = this.value;
    }

    if (this.searchParams) {
      Object.keys(this.searchParams).map((key: any) => {
        queryString += `+${key}:"${this.searchParams[key]}" `;
      });
    } else {
      if (this.field) {
        queryString = `${this.field}:"${this.search}"`;
      } else {
        queryString = `"${this.search}"`;
      }
    }

    this.queryString = queryString;
  }
}
