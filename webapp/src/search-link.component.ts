import {Component, ElementRef, Input, OnInit} from "@angular/core";
import {Router} from "@angular/router";

@Component({
    selector: "search-link",
    template: `<a href="javascript:void(0)" (click)="onClick()">{{value}}</a>`
})
export class SearchLinkComponent implements OnInit {

    @Input() private field:string;
    @Input() private value:string;

    private content:string = "";

    constructor(private router:Router) {
    }

    ngOnInit() {
    }

    onClick() {
        let queryParams:any = this.router.routerState.snapshot.queryParams;

        if (this.field) {
            queryParams["q"] = `${this.field}:"${this.value}"`;
        }
        else {
            queryParams["q"] = `"${this.value}"`;
        }

        this.router.navigate(["/events"], {queryParams: queryParams});
    }

}