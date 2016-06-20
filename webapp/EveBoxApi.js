/* Copyright (c) 2016 Jason Ish
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

class EveBoxApi {

    constructor($http, $window) {
        this.$http = $http;
        this.$window = $window;
        this.baseUrl = window.location.pathname;
    }

    getVersion() {
        return this.$http.get(this.baseUrl + "api/version");
    }

    downloadPcap(what, event) {

        let form = document.createElement("form");
        form.setAttribute("method", "post");
        form.setAttribute("action", "eve2pcap");

        let whatField = document.createElement("input");
        whatField.setAttribute("type", "hidden");
        whatField.setAttribute("name", "what");
        whatField.setAttribute("value", what);
        form.appendChild(whatField);

        let eventField = document.createElement("input");
        eventField.setAttribute("type", "hidden");
        eventField.setAttribute("name", "event");
        eventField.setAttribute("value", JSON.stringify(event));
        form.appendChild(eventField);

        document.body.appendChild(form);
        form.submit();
    }

}

EveBoxApi.$inject = ["$http", "$window"];

angular.module("app").service("EveBoxApi", EveBoxApi);
