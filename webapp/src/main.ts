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

import './polyfills.ts';

import {platformBrowserDynamic} from '@angular/platform-browser-dynamic';
import {enableProdMode} from '@angular/core';
import {AppModule} from './app/app.module';

import "rxjs";

require("!!script-loader!jquery/dist/jquery.min.js");
require("!!script-loader!bootstrap/dist/js/bootstrap.min.js");
require("chart.js");

declare var jQuery:any;
declare var window:any;
declare var localStorage:any;
declare var Chart:any;

if (process.env.ENV === "production") {
    enableProdMode();
}

console.log(process.env);

// Set theme.
switch (localStorage.theme) {
    case "slate":
        require("./styles/evebox-slate.scss");
        break;
    default:
        require("./styles/evebox-default.scss");
        break;
}

// Some chart configuration.
switch (localStorage.theme) {
    case "slate":
        Chart.defaults.global.defaultFontColor = "#fff";
        break;
}

jQuery.getJSON("api/1/config", (config:any) => {
    window.config = config;
    platformBrowserDynamic().bootstrapModule(AppModule);
});
