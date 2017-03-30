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

import {Component, Input} from '@angular/core';
import {ActivatedRoute} from '@angular/router';
import {AppService} from '../app.service';

@Component({
    selector: 'evebox-filter-input',
    template: `
      <form (ngSubmit)="submitFilter()">
        <div class="input-group">
          <input type="text" class="form-control" [(ngModel)]="queryString"
                 placeholder="Filter..." name="queryString"/>
          <div class="input-group-btn">
            <button type="submit" class="btn btn-default">Apply</button>
            <button type="button"
                    class="btn btn-default"
                    (click)="clearFilter()">Clear
            </button>
          </div>
        </div>
      </form>
`
})
export class EveboxFilterInputComponent {

    @Input() queryString: string;

    constructor(private route: ActivatedRoute,
                private appService: AppService) {
    }

    submitFilter() {
        this.appService.updateParams(this.route, {q: this.queryString});
    }

    clearFilter() {
        this.queryString = '';
        this.submitFilter();
    }

}

