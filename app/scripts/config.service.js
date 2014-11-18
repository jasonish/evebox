/* Copyright (c) 2014 Jason Ish
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

'use strict';

(function () {

    /**
     * Centralized configuration.
     *
     * Merges server provided configuration with local storage configuration.
     */
    angular.module("app").factory("Config", function ($http, $location) {

        var defaultConfig = {
            elasticSearch: {
                url: "http://" + window.location.hostname + ":9200",
                size: 100,
                timeout: 6000,
                index: "logstash-*"
            }
        };

        var serverConfig = {};
        var localConfig = {};

        try {
            serverConfig = config;
        }
        catch (error) {
            serverConfig = {};
        }

        if ("config" in localStorage) {
            localConfig = angular.fromJson(localStorage.config);
        }

        var service = {};
        _.merge(service, defaultConfig);
        _.merge(service, serverConfig);
        _.merge(service, localConfig);

        var pruneConfig = function (config, serverConfig) {
            _.forIn(config, function (value, key) {
                if (!_.isFunction(value)) {
                    if (serverConfig[key] != undefined) {
                        if (_.isObject(value)) {
                            pruneConfig(value, serverConfig[key]);
                        }
                        else if (serverConfig[key] == config[key]) {
                            delete(config[key]);
                        }
                    }

                    if (_.size(value) == 0) {
                        delete(config[key]);
                    }
                }
            });
        };

        service.save = function () {
            pruneConfig(service, serverConfig);
            localStorage.config = angular.toJson(service);

            // Force full refresh.
            window.location.reload();
        };

        return service;
    });

})();

