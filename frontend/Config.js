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

import lodash from "lodash";

class Config {

    constructor() {

        this.defaultConfig = {
            elasticSearch: {
                url: `${window.location.protocol}//${window.location.hostname}:${window.location.port}${window.location.pathname}elasticsearch`
            }
        };

        this.config = lodash.cloneDeep(this.defaultConfig)

        if (window.localStorage.config) {
            this.config = Object.assign(this.config,
                JSON.parse(window.localStorage.config));
        }

    }

    getConfig() {
        return this.config;
    }

    save() {
        console.log("Config: Saving.");
        console.log(this.config);
        window.localStorage.config = JSON.stringify(this.config);
    }

    resetToDefaults() {
        delete window.localStorage.config;
        this.config = lodash.cloneDeep(this.defaultConfig);
    }

}

angular.module("app").service("Config", Config);
