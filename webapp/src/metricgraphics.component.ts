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

import {Component, Input, OnChanges} from "@angular/core";
import "metrics-graphics/dist/metricsgraphics.css";
import {EveboxLoadingSpinnerComponent} from "./loading-spinner.component";

let MG = require("metrics-graphics");

@Component({
    selector: "metrics-graphic",
    template: `<div *ngIf="hasData()" class="evebox-metrics-graphic" [id]="graphId"></div>
<div *ngIf="!hasData()" style="text-align: center;">
  <hr/>
  Sorry, there is no data to graph.
  <hr/>
</div>
`,
})
export class EveboxMetricsGraphicComponent implements OnChanges {

    @Input() private graphId:string;
    @Input() private title:string;
    @Input() private data:any[] = [];

    private redraw:boolean = false;

    ngOnChanges() {
        this.redraw = true;
    }

    ngAfterViewChecked() {
        if (this.redraw) {
            this.redraw = false;
            this.doGraphic();
        }
    }

    hasData() {
        return this.data && this.data.length > 0;
    }

    doGraphic() {
        if (this.data && this.data.length > 0) {
            try {
                MG.data_graphic({
                    target: "#" + this.graphId,
                    title: this.title,
                    data: this.data,
                    full_width: true,
                    height: 200,
                    left: 30,
                });
            }
            catch (err) {
                console.log("Failed to draw metrics graphic: " + err);
            }
        }
    }
}