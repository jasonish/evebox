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

import {
  Directive,
  Input,
  ElementRef,
  OnInit,
  OnDestroy,
  OnChanges,
  Output,
  EventEmitter,
  AfterViewChecked,
} from "@angular/core";
import { MousetrapService } from "./mousetrap.service";

declare var jQuery: any;
declare var window: any;

@Directive({
  selector: "[eveboxKeyTable]",
})
export class KeyTableDirective implements OnInit, OnDestroy {
  @Input() private rows: any[] = [];
  @Input() activeRow = 0;
  @Output() activeRowChange: EventEmitter<number> = new EventEmitter<number>();

  constructor(private el: ElementRef, private mousetrap: MousetrapService) {}

  ngOnInit(): any {
    this.mousetrap.bind(this, "j", () => {
      if (this.getActiveRow() < this.getRowCount() - 1) {
        this.setActiveRow(this.getActiveRow() + 1);
      }
    });

    this.mousetrap.bind(this, "k", () => {
      if (this.getActiveRow() > 0) {
        this.setActiveRow(this.getActiveRow() - 1);
      }
    });

    this.mousetrap.bind(this, "G", () => {
      this.setActiveRow(this.getRowCount() - 1);
    });

    this.mousetrap.bind(this, "H", () => {
      this.setActiveRow(0);
    });
  }

  ngOnDestroy(): any {
    this.mousetrap.unbind(this);
  }

  scrollToActive() {
    let error = 175;

    let el =
      this.el.nativeElement.getElementsByTagName("tbody")[0].children[
        this.activeRow
      ];

    let elOffset = el.offsetTop;
    let elHeight = el.scrollHeight;

    let windowOffset = window.pageYOffset;
    let windowHeight = window.innerHeight;

    let elBottom = elOffset + elHeight;
    let windowBottom = windowOffset + windowHeight;

    if (this.activeRow == 0) {
      window.scrollTo(0, 0);
    } else if (elBottom > windowBottom - error - windowHeight * 0.2) {
      let newOffset = windowOffset + elHeight;

      // The first case gives up somewhat of a smooth scroll. But if it
      // doesn't get the active element into view, we need to jump to
      // which is handled by the else.
      if (elBottom < newOffset + windowHeight) {
        window.scrollTo(0, windowOffset + elHeight);
      } else {
        window.scrollTo(0, elBottom);
      }
    } else if (windowOffset > 0) {
      if (elOffset < windowOffset + windowHeight * 0.1) {
        window.scrollTo(0, Math.min(elOffset, windowOffset - elHeight));
      }
    }
  }

  getActiveRow(): number {
    return this.activeRow;
  }

  setActiveRow(activeRow: number) {
    this.activeRow = activeRow;
    this.activeRowChange.emit(this.activeRow);
    this.scrollToActive();
  }

  getRowCount(): number {
    return this.rows.length;
  }
}
