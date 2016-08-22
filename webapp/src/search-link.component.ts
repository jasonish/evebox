import {Component, ElementRef, Input, OnInit} from "@angular/core";
import {Router} from "@angular/router";

@Component({
    selector: "search-link",
    template: `<a href="javascript:void(0)" (click)="onClick()">{{value}}</a>`
})
export class EveboxSearchLinkComponent implements OnInit {

    @Input() private field:string;
    @Input() private value:string;
    @Input() private searchParams:any;
    @Input() private route:string = "/events";

    constructor(private router:Router) {
    }

    ngOnInit() {
    }

    onClick() {
        let queryParams:any = {};
        let queryString = "";

        if (this.searchParams) {
            Object.keys(this.searchParams).map((key:any) => {
                queryString += `+${key}:"${this.searchParams[key]}" `;
            });
        }
        else {
            if (this.field) {
                queryString = `${this.field}:"${this.value}"`;
            }
            else {
                queryString = `"${this.value}"`;
            }
        }

        queryParams["q"] = queryString;

        this.router.navigate([this.route], {queryParams: queryParams});
    }

}