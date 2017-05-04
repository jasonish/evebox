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

import {Injectable} from '@angular/core';

declare var localStorage:any;

function applyTheme(style: string) {
    // Remove the current theme.
    try {
        document.body.removeChild(document.getElementById("theme"));
    }
    catch (e) {
    }

    let node = <HTMLElement>document.createElement('style');
    node.id = "theme";
    node.innerHTML = style;
    document.body.appendChild(node);
}

@Injectable()
export class ThemeService {

    currentTheme() {
        let theme = localStorage.theme;
        if (!theme) {
            theme = "default";
        }
        return theme;
    }

    setTheme(theme:string) {
        switch (theme) {
            case 'slate':
                console.log('Setting theme to slate.');
                applyTheme(require('../../styles/evebox-slate.scss'));
                localStorage.theme = theme;
                break;
            default:
                console.log('Setting theme to default.');
                applyTheme(require('../../styles/evebox-default.scss'));
                localStorage.theme = "default";
                break;
        }
    }

}