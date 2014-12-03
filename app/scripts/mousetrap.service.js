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

    angular.module("app").factory("Mousetrap", function (printf) {

        var debug = false;

        var service = {};
        var bindings = {};

        var log = function (msg) {
            if (debug) {
                console.log(msg);
            }
        };

        service.bind = function (scope, key, callback, help) {
            log(printf("Mousetrap: binding key: {}", key));
            if (help === undefined) {
                console.log(printf("Warning: key {} has no help.", key));
            }

            Mousetrap.unbind(key);

            var bindFunction = function () {
                Mousetrap.bind(key, function (e) {
                    scope.$apply(function () {
                        callback(e);
                    })
                })
            };

            bindings[key] = {
                fn: bindFunction,
                help: help || "document-me"
            };

            bindFunction();

            // Rebinding existing bindings - something is up with Mousetrap.
            for (var binding in bindings) {
                if (binding != key) {
                    bindings[binding].fn();
                }
            }

            scope.$on('$destroy', function () {
                log(printf("Mousetrap: unbinding key: {}", key));
                delete(bindings[key]);
                Mousetrap.unbind(key);
            });
        };

        service.bindings = bindings;

        return service;

    });

})();

