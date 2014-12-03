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

/**
 * Service containing utility functions.
 */
app.factory("Util", function () {

    var service = {};

    /**
     * Return an object as JSON.  Used in EveBox mainly for rendering search
     * results as JSON.
     *
     * Fields prefixed with __ are filtered out as those are internal state
     * variables for the application.
     */
    service.toJson = function (data, pretty) {
        var filtered = _.pick(data, function (value, key) {
            return key.substring(0, 2) != "__";
        });
        return angular.toJson(filtered, pretty);
    };

    /**
     * Format a string.
     *
     * Example: formatString("This is a {} {}.", "format", "string");
     */
    service.printf = function (format) {
        var buf = arguments[0];
        var args = Array.prototype.slice.call(arguments, 1);

        for (var i = 0; i < args.length; i ++) {
            buf = buf.replace("{}", args[i]);
        }

        return buf;
    };

    /**
     * Convert an alert severity into a Bootstrap class for colorization.
     */
    service.severityToBootstrapClass = function (severity, prefix) {
        if (prefix === undefined) {
            prefix = "";
        }
        switch (severity) {
            case 1:
                return prefix + "danger";
                break;
            case 2:
                return prefix + "warning";
                break;
            default:
                return prefix + "info";
        }
    };

    service.timestampToFloat = function (timestamp) {
        var usecs = timestamp.match(/\.(\d+)/)[1] / 1000000;
        var secs = moment(timestamp).unix();
        return secs + usecs;
    };

    service.isBase64 = function (str) {
        try {
            atob(str);
            return true;
        }
        catch (error) {
            return false;
        }
    };

    service.base64ToHexArray = function (str) {
        for (var i = 0, bin = atob(str.replace(/[ \r\n]+$/, "")), hex = []; i
        < bin.length; ++ i) {
            var tmp = bin.charCodeAt(i).toString(16);
            if (tmp.length === 1)
                tmp = "0" + tmp;
            hex[hex.length] = tmp;
        }
        return hex;
    };

    return service;
});

angular.module("app").factory("printf", function () {

    return function (format) {
        var buf = arguments[0];
        var args = Array.prototype.slice.call(arguments, 1);

        for (var i = 0; i < args.length; i ++) {
            buf = buf.replace("{}", args[i]);
        }

        return buf;
    };

});

/**
 * Elastic Search operations.
 */
app.factory("ElasticSearch", function ($http, Config, printf) {

    var service = {};

    var esUrl = Config.elasticSearch.url;

    service.logFailure = function (failure) {
        console.log("elastic search server failure: " + failure);
    };

    /**
     * Search.
     */
    service.search = function (query) {
        var url = printf("{}/{}/_search?refresh=true",
            esUrl, Config.elasticSearch.index);
        return $http.post(url, query);
    };

    service.bulk = function (request) {
        var url = Config.elasticSearch.url + "/_bulk?refresh=true";
        return $http.post(url, request);
    };

    service.update = function (index, type, id, request) {
        var url = printf("{}/{}/{}/{}/_update?refresh=true",
            esUrl, index, type, id);
        return $http.post(url, request);
    };

    service.delete = function (index, type, id) {
        var url = printf("{}/{}/{}/{}?refresh=true",
            esUrl, index, type, id);
        return $http.delete(url);
    };

    service.deleteByQuery = function (request) {
        var url = printf("{}/{}/_query?refresh=true",
            esUrl, Config.elasticSearch.index);
        return $http.delete(url, {data: request});
    };

    /**
     * Get/search for a record by ID.
     *
     * Used for getting a single event by ID, but may return multiple results.
     */
    service.searchEventById = function (id) {
        var request = {
            query: {
                filtered: {
                    filter: {
                        term: {
                            "_id": id
                        }
                    }
                }
            }
        };
        return service.search(request);
    };

    /**
     * Bulk delete events.
     *
     * @param events The list of events to delete.
     */
    service.deleteEvents = function (events) {
        var request = events.map(function (event) {
                return angular.toJson({
                    delete: {
                        _index: event._index,
                        _type: event._type,
                        _id: event._id
                    }
                });
            }).join("\n") + "\n";
        return service.bulk(request);
    };

    service.bulkRemoveTag = function (events, tag) {
        var request = events.map(function (event) {
            return [
                angular.toJson({
                    update: {
                        _index: event._index,
                        _type: event._type,
                        _id: event._id
                    }
                }),
                angular.toJson({
                    lang: "groovy",
                    script: "ctx._source.tags.remove(tag)",
                    params: {
                        "tag": tag
                    }
                })
            ];
        });
        return service.bulk(_.flatten(request).join("\n") + "\n");
    };

    service.addTag = function (doc, tag) {
        var script = 'if (ctx._source.tags) {' +
            'ctx._source.tags.contains(tag) || ctx._source.tags.add(tag);' +
            '}' +
            'else {' +
            'ctx._source.tags = [tag]' +
            '}';
        var request = {
            "lang": "groovy",
            //script: "ctx._source.tags.contains(tag) || ctx._source.tags.add(tag)",
            script: script,
            params: {
                "tag": tag
            }
        };
        return service.update(doc._index, doc._type, doc._id, request);
    };

    service.removeTag = function (doc, tag) {
        var request = {
            lang: "groovy",
            script: "ctx._source.tags.remove(tag)",
            params: {
                "tag": tag
            }
        };
        return service.update(doc._index, doc._type, doc._id, request);
    };

    return service;

});

angular.module("app").factory("NotificationService", function ($timeout) {

    var service = {};

    service.queue = [];

    service.add = function (level, message) {
        var entry = {
            level: level,
            message: message
        };

        service.queue.push(entry);

        $timeout(function () {
            _.remove(service.queue, function (item) {
                return item === entry;
            })
        }, 1500);
    };

    return service;
});
