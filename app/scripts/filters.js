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

(function() {

    angular.module("app").filter("formatIpAddress", [function() {

        return function(addr) {
            if (addr === undefined) {
                return "";
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

    }]);

    /**
     * Based on the event type, return a suitable event title.
     */
    angular.module("app").filter("eventTitle", [function(Util) {

        return function(event) {
            if (event._source.alert) {
                return event._source.alert.signature;
            }
            else if (event._source.dns) {
                return Util.printf("{}: {}",
                    event._source.event_type.toUpperCase(),
                    event._source.dns.rrname);
            }
            else if (event._source.tls) {
                return Util.printf("{}: {}",
                    event._source.event_type.toUpperCase(),
                    event._source.tls.subject);
            }
            else if (event._source.http) {
                return Util.printf("{}: {} {}",
                    event._source.event_type.toUpperCase(),
                    event._source.http.http_method,
                    event._source.http.hostname);
            }
            else {
                return event._source.event_type.toUpperCase();
            }

        };

    }]);

    angular.module("app").filter("eventTitleClass", [function() {

        return function(event) {
            if (event._source.alert) {
                switch (event._source.alert.severity) {
                    case 1:
                        return "alert-danger";
                        break;
                    case 2:
                        return "alert-warning";
                        break;
                    default:
                        return "alert-info";
                }
            }
            else {
                return "alert-info";
            }
        };

    }]);

    angular.module("app").filter("formatTimestamp", [function() {

        return function(timestamp) {
            return moment(timestamp).format();
        }

    }]);

    angular.module("app").filter("severityClass", ["Util", function(Util) {

        return function(input) {
            return Util.severityToBootstrapClass(input);
        }

    }]);

    /**
     * Colourize a JSON string.
     *
     * Based on code snippet from:
     * http://stackoverflow.com/questions/4810841/how-can-i-pretty-print-json-using-javascript
     */
    angular.module("app").filter("colourizeJson",
        ["$sce", "Util", function($sce, Util) {
            return function(json) {
                return $sce.trustAsHtml(Util.colourizeJson(json));
            };
        }]);

    angular.module("app").filter("formatEventDescription",
        ["$sce", "Util", function($sce, Util) {

            return function(event) {

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
                            return Util.colourizeJson(
                                angular.toJson(
                                    event._source[event._source.event_type],
                                    true));
                        }
                    }
                }

            }

        }]);

})();

