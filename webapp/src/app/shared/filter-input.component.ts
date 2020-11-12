// Copyright (C) 2016-2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

import { Component, Input, OnDestroy, OnInit } from "@angular/core";
import { ActivatedRoute } from "@angular/router";
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
        this.appService.updateParams(this.route, {q: this.queryString});
    }

    clearFilter(): void {
        this.queryString = "";
        this.submitFilter();
    }
}

