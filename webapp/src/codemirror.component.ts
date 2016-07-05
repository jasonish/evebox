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

import {Component, OnInit, Input} from "@angular/core";

var codemirror = require("codemirror");

require("codemirror/mode/javascript/javascript.js");

// Folding support. Doesn't work too well.
// require("codemirror/addon/fold/foldcode.js");
// require("codemirror/addon/fold/brace-fold.js");
// require("codemirror/addon/fold/foldgutter.js");
// require("codemirror/addon/fold/foldgutter.css");

/**
 * This component implements the CodeMirror editor.
 *
 * This editor is currently not being used, but I've left the component here
 * for possible future use instead of the Ace editor.
 */
@Component({
    selector: "codemirror",
    template: `<div id="codemirror-editor"></div>`
})
export class CodemirrorComponent implements OnInit {

    @Input("mode") textMode:string;
    @Input("text") text:string = "";

    private editor:any;

    ngOnInit() {
        this.refresh();
    }

    refresh() {

        let mode:any = false;

        switch (this.textMode) {
            case "json":
                mode = "application/json";
                break;
        }

        this.editor = codemirror(document.getElementById("codemirror-editor"), {
            lineNumbers: true,
            lineWrapping: true,
            value: this.text,
            mode: mode,
            readOnly: true
        });
        this.editor.setSize("100%", "100%");

    }
}