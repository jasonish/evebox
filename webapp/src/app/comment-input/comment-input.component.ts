import {Component, EventEmitter, OnInit, Output} from '@angular/core';

@Component({
    selector: 'app-comment-input',
    templateUrl: './comment-input.component.html',
})
export class CommentInputComponent implements OnInit {

    @Output("on-close") public onClose = new EventEmitter<any>();
    @Output("on-submit") public onSubmit = new EventEmitter<any>();

    public comment: string = "";

    constructor() {
    }

    ngOnInit() {
    }

    close() {
        this.onClose.emit();
    }

    submitComment() {
        this.onSubmit.emit(this.comment);
    }

}
