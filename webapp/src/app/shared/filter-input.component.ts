// Copyright (C) 2016-2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE
// OR OTHER DEALINGS IN THE SOFTWARE.

import { Component, Input, OnDestroy, OnInit } from "@angular/core";
import { ActivatedRoute, Router } from "@angular/router";
import { AppService } from "../app.service";
import { MousetrapService } from "../mousetrap.service";

@Component({
    selector: "evebox-filter-input",
    template: `
        <form (ngSubmit)="submitFilter()">
            <div class="input-group">
                <input id="filterInput" type="text" class="form-control" [(ngModel)]="queryString"
                       placeholder="Filter..." name="queryString"/>
                <span class="input-group-append">
            <button type="submit"
                    class="btn btn-secondary">Apply</button>
            <button type="button"
                    class="btn btn-secondary"
                    (click)="clearFilter()">Clear
            </button>
          </span>
            </div>
        </form>
    `
})
export class EveboxFilterInputComponent implements OnInit, OnDestroy {

    @Input() queryString: string;

    constructor(private route: ActivatedRoute,
                private mousetrap: MousetrapService,
                private router: Router,
                private appService: AppService) {
    }

    ngOnInit(): void {
        this.mousetrap.bind(this, "/", () => {
            document.getElementById("filterInput").focus();
        });
    }

    ngOnDestroy(): void {
        this.mousetrap.unbind(this);
    }

    submitFilter(): void {
        document.getElementById("filterInput").blur();
        const queryParams: any = {};
        if (this.queryString !== "") {
            queryParams.q = this.queryString;
        }
        this.router.navigate([], {
            queryParams,
        });
    }

    clearFilter(): void {
        this.queryString = "";
        this.submitFilter();
    }
}

