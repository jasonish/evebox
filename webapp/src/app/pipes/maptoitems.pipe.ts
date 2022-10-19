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

import { Pipe, PipeTransform } from "@angular/core";

/**
 * Example usage:
 *
 *     <div *ngFor="let item of event._source.http | mapToItems">
 *         {{item.key}} = {{item.val}}
 *     </div>
 */
@Pipe({
    name: "mapToItems",
})
export class EveboxMapToItemsPipe implements PipeTransform {
    flatten(object: any) {
        let result = {};

        for (let x in object) {
            if (!object.hasOwnProperty(x)) continue;

            if (typeof object[x] == "object") {
                let flattened = this.flatten(object[x]);
                for (let y in flattened) {
                    if (!flattened.hasOwnProperty(y)) {
                        continue;
                    }
                    result[x + "." + y] = flattened[y];
                }
            } else {
                result[x] = object[x];
            }
        }

        return result;
    }

    transform(value: any, args: any): any {
        // Should make optional.
        value = this.flatten(value);

        return Object.keys(value).map((key) => {
            return {
                key: key,
                val: value[key],
            };
        });
    }
}
