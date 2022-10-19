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

import { Component, Input } from "@angular/core";
import {
    animate,
    state,
    style,
    transition,
    trigger,
} from "@angular/animations";

@Component({
    selector: "loading-spinner",
    template: `<div
        *ngIf="active"
        [@eveboxSpinnerTrigger]="active ? 'true' : 'false'"
    >
        <i
            class="fa fa-spinner fa-pulse evebox-loading-spinner"
            [ngStyle]="{ 'font-size': fontSize + 'px' }"
        ></i>
    </div>`,
    animations: [
        trigger("eveboxSpinnerTrigger", [
            state(
                "void",
                style({
                    opacity: 0,
                })
            ),
            state(
                "*",
                style({
                    opacity: 1,
                })
            ),
            transition("void => *", animate("500ms")),
            transition("* => void", animate("500ms")),
        ]),
    ],
})
export class EveboxLoadingSpinnerComponent {
    @Input("loading") active = false;
    @Input("fontSize") fontSize = 300;
}
