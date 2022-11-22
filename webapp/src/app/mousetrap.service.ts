/* Copyright (c) 2014-2016 Jason Ish
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

declare function require(name: string);

import { Injectable, NgZone } from "@angular/core";

let mousetrap = require("mousetrap/mousetrap");

declare var Mousetrap: any;

@Injectable()
export class MousetrapService {
  private bindings: any[] = [];
  private anyBindings: any[] = [];

  constructor(private ngZone: NgZone) {
    let mouseTrapService = this;

    Mousetrap.prototype.handleKey = function () {
      let self = this;
      self._handleKey.apply(this, arguments);

      mouseTrapService.anyBindings.forEach((binding) => {
        binding.handler();
      });
    };
  }

  bindAny(component: any, handler: any) {
    this.anyBindings.push({
      component: component,
      handler: handler,
    });
  }

  bind(component: any, key: string, handler: any) {
    mousetrap.bind(key, (e: any) => {
      this.ngZone.run(() => {
        e.preventDefault();
        handler();
      });
    });
    this.bindings.push({
      component: component,
      key: key,
      handler: handler,
    });

    this.rebind();
  }

  rebind() {
    this.bindings.forEach((binding) => {
      mousetrap.unbind(binding.key);
      mousetrap.bind(binding.key, (e: any) => {
        this.ngZone.run(() => {
          e.preventDefault();
          binding.handler();
        });
      });
    });
  }

  unbind(component: any) {
    this.bindings.forEach((binding) => {
      if (binding.component == component) {
        mousetrap.unbind(binding.key);
      }
    });
    this.bindings = this.bindings.filter((binding) => {
      return binding.component != component;
    });
  }
}
