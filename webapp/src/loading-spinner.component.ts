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
    Component, Input, style, state, animate,
    transition, trigger, ElementRef, OnInit
} from "@angular/core";

@Component({
    selector: "loading-spinner",
    template: `<div [@visibleState]="loading ? 'true' : 'false'">
    <i class="fa fa-spinner fa-pulse evebox-loading-spinner"
    [hidden]="!isVisible()"
    [ngStyle]="{'font-size': fontSize + 'px'}"></i>
</div>`,
    animations: [
        trigger('visibleState', [
                state("false", style({
                    opacity: '0',
                    //visibility: "hidden",
                })),
                state("true", style({
                    opacity: '1.0',
                    visibility: "visible",
                })),
                transition('false => true', animate('500ms ease-out')),
                transition('true => false', animate('1000ms ease-out'))
            ]
        )
    ]
})
export class EveboxLoadingSpinnerComponent {

    @Input("loading") private loading:boolean = false;
    @Input("fontSize") private fontSize:number = 300;

    constructor(private element:ElementRef) {
    }

    isVisible() {
        let opacity = this.element.nativeElement.children[0].style.opacity;
        if (opacity == "0") {
            return false;
        }
        return true;
    }

}