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

import {Component, OnInit, Input} from '@angular/core';

import 'brace';
import 'brace/mode/json';

declare var ace: any;
declare var $: any;

@Component({
    selector: 'ace-editor',
    template: `<div id="ace-editor"></div>`
})
export class AceEditor implements OnInit {

    // The text to show in the editor.
    @Input() private value: string;

    // Read only.
    private readOnly = false;

    // Mode (json, etc.)
    @Input() private mode: string;

    // Wrap text or not.
    @Input('wrap') private wrap = true;

    // The Ace editor instace.
    private editor: any;

    ngOnInit() {

        this.editor = ace.edit('ace-editor');

        // Suppresses a deprecation warning.
        this.editor.$blockScrolling = Infinity;

        this.editor.setReadOnly = this.readOnly;
        this.editor.getSession().setUseWrapMode(this.wrap);

        if (this.mode) {
            this.editor.getSession().setMode('ace/mode/' + this.mode);
        }

        this.editor.setValue(this.value, -1);

        this.resize();
    }

    resize() {
        let height = this.editor.getSession().getScreenLength()
            * this.editor.renderer.lineHeight
            + this.editor.renderer.scrollBar.getWidth()
            + 30; // For some extra bottom buffer.
        $('#ace-editor').height(height.toString() + 'px');
        this.editor.resize();
    };
}