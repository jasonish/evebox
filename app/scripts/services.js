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

    service.save = function () {

        if (localConfig.elasticSearch === undefined) {
            localConfig.elasticSearch = {};
        }

        if (localConfig.elasticSearch != undefined && service.elasticSearch.url == serverConfig.elasticSearch.url) {
            console.log("Deleting current elasticSearch.url");
            delete(localConfig.elasticSearch.url);
        }
        else {
            if (localConfig.elasticSearch === undefined) {
                localConfig.elasticSearch = {};
            }
            localConfig.elasticSearch.url = service.elasticSearch.url;
        }

        if (service.elasticSearch.size == serverConfig.elasticSearch.size) {
            delete(localConfig.elasticSearch.size);
        }
        else {
            localConfig.elasticSearch.size = service.elasticSearch.size;
        }

        localStorage.config = angular.toJson(localConfig);

        // Force full refresh.
        window.location.reload();
    };

    return service;
});

/**
 * Elastic Search operations.
 */
app.factory("ElasticSearch", function ($resource, Config) {

    var service = {};

    service.resource = $resource(Config.elasticSearch.url, null, {

        "update": {
            method: "POST",
            url: Config.elasticSearch.url + "/:index/:type/:id/_update?refresh=true",
            params: {
                index: "@index",
                type: "@type",
                id: "@id"
            }
        },

        "search": {
            method: "POST",
            url: Config.elasticSearch.url + "/_all/_search?refresh=true"
        },

        "delete": {
            method: "DELETE",
            url: Config.elasticSearch.url + "/:index/:type/:id?refresh=true"
        }

    });
    service.update = service.resource.update;
    service.search = service.resource.search;

    service.logFailure = function (failure) {
        console.log("elastic search server failure: " + failure);
    }

    service.delete = function (doc, success, fail) {
        if (success == undefined) {
            success = function () {
            };
        }
        if (fail == undefined) {
            fail = service.logFailure;
        }
        return service.resource.delete(
            {index: doc._index, type: doc._type, id: doc._id}, success, fail);
    };

    service.updateTags = function (doc) {
        return service.resource.update({index: doc._index, type: doc._type, id: doc._id},
            {doc: {tags: doc._source.tags}});
    }

    service.removeTag = function (doc, tag, success, fail) {
        return service.resource.update({index: doc._index, type: doc._type, id: doc._id},
            {script: "ctx._source.tags.remove('" + tag + "')"}, success, fail);
    }

    service.queryStringSearch = function (queryString, params) {

        var size = Config.elasticSearch.size;
        var page = 0

        if (params.size != undefined) {
            size = params.size;
        }
        if (params.page != undefined) {
            page = params.page;
        }

        return service.resource.search({
            query: {
                filtered: {
                    query: {
                        query_string: {
                            query: queryString
                        }
                    }
                }
            },
            size: size,
            from: size * page,
            sort: [
                {"@timestamp": {order: "desc"}}
            ]
        });
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
        Mousetrap.bind(key, callback);
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
            service.scopeBindings[scope.$id] = [];
        }

        // Something is up with Mousetrap bindings, rebinding existing
        // bindings seems to fix it.
        for (scopeId in service.scopeBindings) {
            _.forEach(service.scopeBindings[scopeId], function (binding) {
                Mousetrap.bind(binding.key, binding.callback);
            });
        }
    };

    return service;

});