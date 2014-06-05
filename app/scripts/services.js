/*
 * Copyright (c) 2014 Jason Ish
 * All rights reserved.
 */

/*
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

var NAV_BAR_HEIGHT = 60;

/**
 * Centralized configuration.
 *
 * Merges server provided configuration with local storage configuration.
 */
app.factory("Config", function ($http, $location) {

    var serverConfig = {};
    var localConfig = {};

    try {
        serverConfig = config;
    }
    catch (error) {
        serverConfig = {
            elasticSearch: {
                url: "http://" + window.location.hostname + ":9200",
                size: 100
            }
        }
    }

    if ("config" in localStorage) {
        localConfig = angular.fromJson(localStorage.config);
    }

    service = {};
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
     * @param callback Callback on response.
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
 * A service for keyboard bindings (wrapping Mousetrap) that will track
 * the scope a binding was created in for per scope cleanup.
 */
app.factory("Keyboard", function () {

    var service = {};
    service.scopeBindings = {};

    service.scopeBind = function (scope, key, callback) {
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