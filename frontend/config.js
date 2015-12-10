/* Copyright (c) 2014-2015 Jason Ish
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

import angular from "angular";
import lodash from "lodash";
import * as appEvents from "./app-events";

(function () {

    angular.module("app").factory("Config", Config);

    Config.$inject = [];

    function Config() {

        let defaultConfig = {
            elasticSearch: {
                url: `${window.location.protocol}//${window.location.hostname}:${window.location.port}${window.location.pathname}elasticsearch`
            }
        };

        let service = {
            save: save,
            getConfig: getConfig,
            resetToDefaults: resetToDefaults,
            config: lodash.cloneDeep(defaultConfig)
        };

        if (window.localStorage.config) {
            service.config = Object.assign(service.config,
                JSON.parse(window.localStorage.config));
        }

        return service;

        function getConfig() {
            return service.config;
        }

        function save() {
            console.log("Config: Saving.");
            console.log(service.config);
            window.localStorage.config = JSON.stringify(service.config);
        }

        function resetToDefaults() {
            delete window.localStorage.config;
            service.config = lodash.cloneDeep(defaultConfig);
        }

    }

})();

