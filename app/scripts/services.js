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

/**
 * Centralized configuration.
 *
 * Merges server provided configuration with local storage configuration.
 */
app.factory("Config", function ($http, $location) {

    var defaultConfig = {
        "elasticSearch": {
            "url": "http://" + window.location.hostname + ":9200",
            "size": 100,
            "timeout": 6000
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

    service = {};
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
     * String formatter.
     *
     * Example: formatString("Numbers {0}, {1}, {2}.", "one", "two", "three");
     *
     * Based on:
     * http://stackoverflow.com/questions/610406/javascript-equivalent-to-printf-string-format/4673436#4673436
     */
    service.formatString = function (format) {
        var args = Array.prototype.slice.call(arguments, 1);
        return format.replace(/{(\d+)}/g, function (match, number) {
            return typeof args[number] != 'undefined' ? args[number] : match;
        });
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

    /**
     * Check if an element is currently within the visible area of the window.
     *
     * From http://stackoverflow.com/questions/487073/check-if-element-is-visible-after-scrolling.
     */
    service.isScrolledIntoView = function (element) {
        var docViewTop = $(window).scrollTop();
        var docViewBottom = docViewTop + $(window).height();
        var elemTop = $(element).offset().top;
        var elemBottom = elemTop + $(element).height();
        return ((elemBottom < docViewBottom) && (elemTop >= docViewTop));
    };

    service.scrollElementIntoView = function (element) {
        if (!service.isScrolledIntoView(element)) {
            $(window).scrollTop(element.position().top - ($(window).height() / 2));
        }
    };

    /**
     * Print an IP address.  Really only used to shorten up IPv6 addresses.
     */
    service.printIpAddress = function (addr) {
        if (addr === undefined) {
            return "";
        }
        addr = addr.replace(/0000/g, "");
        while (addr.indexOf(":0:") > -1) {
            addr = addr.replace(/:0:/g, "::");
        }
        addr = addr.replace(/:::+/g, "::");
        while (addr != (addr = addr.replace(/:0+/g, ":")));
        return addr;
    };

    service.timestampToFloat = function (timestamp) {
        var usecs = timestamp.match(/\.(\d+)/)[1] / 1000000;
        var secs = moment(timestamp).unix();
        return secs + usecs;
    };

    return service;
});

/**
 * Elastic Search operations.
 */
app.factory("ElasticSearch", function ($http, Config) {

    var service = {};

    service.logFailure = function (failure) {
        console.log("elastic search server failure: " + failure);
    };

    /**
     * Search.
     */
    service.search = function (query) {
        var url = Config.elasticSearch.url + "/_all/_search?refresh=true";
        return $http.post(url, query);
    };

    service.bulk = function (request) {
        var url = Config.elasticSearch.url + "/_bulk?refresh=true";
        return $http.post(url, request);
    };

    service.msearch = function (request) {
        var url = Config.elasticSearch.url + "/_msearch?refresh=true";
        return $http.post(url, request);
    };

    service.update = function (index, type, id, request) {
        var url = Config.elasticSearch.url + "/" + index +
            "/" + type +
            "/" + id +
            "/_update?refresh=true";
        return $http.post(url, request);
    };

    service.delete = function (index, type, id) {
        var url = Config.elasticSearch.url + "/" + index + "/" + type + "/" + id + "?refresh=true";
        return $http.delete(url);
    };

    service.deleteByQuery = function (request) {
        var url = Config.elasticSearch.url + "/_all/_query?refresh=true";
        return $http.delete(url, {data: request});
    }

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
        var request = {
            script: "ctx._source.tags.contains(tag) ? (ctx.op = \"none\") : ctx._source.tags += tag",
            params: {
                "tag": tag
            }
        };
        return service.update(doc._index, doc._type, doc._id, request);
    };

    service.removeTag = function (doc, tag) {
        var request = {
            script: "ctx._source.tags.remove(tag)",
            params: {
                "tag": tag
            }
        };
        return service.update(doc._index, doc._type, doc._id, request);
    };

    return service;

});

/**
 * EventRepository service.
 *
 * The idea of this service is provide a level of abstraction over
 * ElasticSearch.
 */
app.factory("EventRepository", function (ElasticSearch, $q) {

    var service = {};

    /**
     * Delete the provided event.
     *
     * @param event The event to delete.
     * @returns HttpPromise.
     */
    service.deleteEvent = function (event) {
        return ElasticSearch.delete(event._index, event._type, event._id);
    };

    /**
     * Remove a tag from an event.
     *
     * @param event Event to remove tag from.
     * @param tag The tag to remove.
     * @returns HttpPromise.
     */
    service.removeTag = function (event, tag) {
        return ElasticSearch.removeTag(event, tag)
            .success(function (response) {
                _.remove(event._source.tags, function (t) {
                    return t === tag;
                });
            });
    };

    /**
     * Toggle a tag on event - remove it if it exists, otherwise add it.
     *
     * @param event Event to toggle tag on.
     * @param tag Tag to toggle.
     * @returns HttpPromise.
     */
    service.toggleTag = function (event, tag) {
        if (_.indexOf(event._source.tags, tag) > -1) {
            return service.removeTag(event, tag);
        }
        else {
            return ElasticSearch.addTag(event, tag)
                .success(function (response) {
                    event._source.tags.push(tag);
                });
        }
    };

    /**
     * Toggle the "starred" tag on an event.
     */
    service.toggleStar = function (event) {
        return service.toggleTag(event, "starred");
    };

    return service;

});

/**
 * A service for keyboard bindings (wrapping Mousetrap) that will track
 * the scope a binding was created in for per scope cleanup.
 */
app.factory("Keyboard", function () {

    var service = {};
    service.scopeBindings = {};

    service.scopeBind = function (scope, key, callback) {
        Mousetrap.unbind(key);
        Mousetrap.bind(key, function (e) {
            callback(e);
        });
        if (!(scope.$id in service.scopeBindings)) {
            service.scopeBindings[scope.$id] = [];
        }
        service.scopeBindings[scope.$id].push({key: key, callback: callback});
    };

    service.resetScope = function (scope) {
        if (scope.$id in service.scopeBindings) {
            _.forEach(service.scopeBindings[scope.$id], function (binding) {
                Mousetrap.unbind(binding.key);
            });
            delete(service.scopeBindings[scope.$id]);
        }

        // Something is up with Mousetrap bindings, rebinding existing
        // bindings seems to fix it.
        for (var scopeId in service.scopeBindings) {
            _.forEach(service.scopeBindings[scopeId], function (binding) {
                Mousetrap.bind(binding.key, binding.callback);
            });
        }
    };

    return service;

});

app.factory("Cache", function () {

    var service = {
        caches: {}
    };

    // Return a cache of the given name.
    service.get = function (name) {
        if (service.caches[name] === undefined) {
            service.caches[name] = {};
        }
        return service.caches[name];
    };

    return service;

});

app.factory("NotificationMessageService", function ($timeout) {

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