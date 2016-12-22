import './polyfills.ts';

import {platformBrowserDynamic} from '@angular/platform-browser-dynamic';
import {enableProdMode} from '@angular/core';
import {environment} from './environments/environment';
import {AppModule} from './app/app.module';

import "rxjs";
require("!!script!jquery/dist/jquery.min.js");
require("!!script!bootstrap/dist/js/bootstrap.min.js");

declare var jQuery:any;
declare var window:any;
declare var localStorage:any;

if (environment.production) {
    enableProdMode();
}

// Set theme.
switch (localStorage.theme) {
    case "slate":
        require("./styles/evebox-slate.scss");
        break;
    default:
        require("./styles/evebox-default.scss");
        break;
}

jQuery.getJSON("/api/1/config", (config:any) => {
    window.config = config;
    platformBrowserDynamic().bootstrapModule(AppModule);
});

