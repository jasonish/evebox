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

    angular.module("app").filter("formatIpAddress", [function () {

        return function (addr) {
            if (addr === undefined) {
                return "";
            }
            addr = addr.replace(/0000/g, "");
            while (addr.indexOf(":0:") > - 1) {
                addr = addr.replace(/:0:/g, "::");
            }
            addr = addr.replace(/:::+/g, "::");
            while (addr != (addr = addr.replace(/:0+/g, ":")))
                ;
            return addr;
        }

    }]);

    angular.module("app").filter("formatTimestamp", [function () {

        return function (timestamp) {
            return moment(timestamp).format();
        }

    }]);

    angular.module("app").filter("severityClass", ["Util", function (Util) {

        return function (input) {
            return Util.severityToBootstrapClass(input);
        }

    }]);

    angular.module("app").filter("formatEventDescription",
        ["$sce", "Util", function ($sce, Util) {

            return function (event) {

                if (event._source) {
                    switch (event._source.event_type) {
                        case "alert":
                        {
                            var formatted = event._source.alert.signature;
                            if (event._source.alert.category) {
                                formatted
                                    += Util.printf(' <small class="text-muted">[{}]</small>',
                                    event._source.alert.category);
                            }
                            return $sce.trustAsHtml(formatted);
                        }
                        default:
                        {
                            var parts = [];
                            _.forIn(event._source[event._source.event_type],
                                function (value, key) {
                                    parts.push('<span style="color: #808080;">'
                                    +
                                    key +
                                    ':</span> ' +
                                    '<span style="word-break: break-all;">' +
                                    value +
                                    '</span>');
                                });
                            var msg = parts.join("; ");
                            return $sce.trustAsHtml(msg);

                        }
                    }
                }

            }

        }]);

})();

