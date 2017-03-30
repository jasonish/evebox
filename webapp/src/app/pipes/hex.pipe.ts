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

import {Pipe, PipeTransform} from '@angular/core';

/**
 * Pipe to convert input to hex.
 *
 * The hex is printable, with 16 bytes per line.
 * Input data should be binary.
 */
@Pipe({
    name: 'hex'
})
export class EveboxHexPipe implements PipeTransform {

    transform(value: any, args: any): any {

        let hex: any = [];

        for (let i = 0; i < value.length; ++i) {
            let tmp = value.charCodeAt(i).toString(16);
            if (tmp.length === 1) {
                tmp = '0' + tmp;
            }
            hex[hex.length] = tmp;
        }

        let output = '';
        for (let i = 0; i < hex.length; i++) {
            if (i > 0 && i % 16 == 0) {
                output += '\n';
            }
            output += hex[i] + ' ';
        }

        return output;
    }

}