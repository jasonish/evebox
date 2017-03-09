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

import {Pipe, PipeTransform} from "@angular/core";

declare var window:any;

let randomMap = {};

/**
 * IP address formatting pipe.
 *
 * To randomize IP addresses set "window.randomizeIp = true" on the window
 * object.
 */
@Pipe({
    name: "eveboxFormatIpAddress"
})
export class EveboxFormatIpAddressPipe implements PipeTransform {

    transform(addr:string) {
        if (addr === undefined) {
            return "";
        }

        if (window.randomizeIp) {
            return this.getRandomIp(addr);
        }

        addr = addr.replace(/0000/g, "");
        while (addr.indexOf(":0:") > -1) {
            addr = addr.replace(/:0:/g, "::");
        }
        addr = addr.replace(/:::+/g, "::");
        while (addr != (addr = addr.replace(/:0+/g, ":")))
            ;
        return addr;
    }

    getRandomIp(addr:string) {
        if (randomMap[addr]) {
            return randomMap[addr];
        }

        let randomAddr = "10." +
                Math.round(Math.random() * 256) + "." +
                Math.round(Math.random() * 256) + "." +
                Math.round(Math.random() * 256);

        randomMap[addr] = randomAddr;

        return randomAddr;
    }
}