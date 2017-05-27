/* Copyright (c) 2017 Jason Ish
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

import {Pipe, PipeTransform} from '@angular/core';

@Pipe({
    name: 'ruleHighlight'
})
export class RuleHighlightPipe implements PipeTransform {

    transform(value: any, args?: any): any {

        // First encode html.
        value = value.replace("<", "&___lt___");
        value = value.replace(">", "&___gt___");

        value = value.replace(
                /^([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+/,
                `<span class="rule-header-action">$1</span> 
                 <span class="rule-header-proto">$2</span>
                 <span class="rule-header-addr">$3</span>
                 <span class="rule-header-port">$4</span> 
                 <span class="rule-header-direction">$5</span> 
                 <span class="rule-header-addr">$6</span>
                 <span class="rule-header-port">$7</span> `);

        value = value.replace(/:([^;]+)/g, `:<span class="rule-keyword-value">$1</span>`);
        value = value.replace(/(\w+\:)/g, `<span class="rule-keyword">$1</span>`);

        // Catch keywords without a value.
        value = value.replace(/(;\s*)(\w+;)/g, `$1<span class="rule-keyword">$2</span>`);

        value = value.replace("&___lt___", "&lt;");
        value = value.replace("&___gt___", "&gt;");

        return value;
    }

}
