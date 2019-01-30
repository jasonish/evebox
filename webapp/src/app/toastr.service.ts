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

declare function require(name:string);

import {Injectable} from '@angular/core';

let toastr = require('toastr');

// Fixes toastr when Bootstrap 4.2 is present.
toastr.options.toastClass = 'toastr';

export interface ToastrOptions {
    title?: string;
    closeButton?: boolean;

    // How long the toast will be displayed until the user interacts with it.
    // 0 to disable, however will still timeout after the user hovers over it.
    timeOut?: number;

    // How to the toast will be displayed after user interaction, like hovering.
    extendedTimeOut?: number;

    preventDuplicates?: boolean;
}

@Injectable()
export class ToastrService {

    warning(msg: any, options: ToastrOptions = {}) {
        toastr.warning(msg, options.title, options);
    }

    error(msg: any, options: ToastrOptions = {}) {
        toastr.error(msg, options.title, options);
    }

}
