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

app.controller("RecordController", function ($scope, $routeParams, Util,
    ElasticSearch) {

    // Export some functions to $scope.
    $scope.Util = Util;

    ElasticSearch.searchEventById($routeParams.id)
        .success(function (response) {
            $scope.response = response;
            $scope.hits = response.hits;
        });

});

app.controller("ArchiveByQueryProgressModal", function ($scope, ElasticSearch,
    inboxScope, Util, $timeout) {

    modalScope = $scope;

    $scope.numberToArchive = inboxScope.hits.total;
    $scope.numberArchived = 0;
    $scope.error = undefined;

    var latestTimestamp = inboxScope.hits.hits[0]._source["@timestamp"];
    var searchRequest = {
        query: {
            filtered: {
                query: {
                    query_string: {
                        query: inboxScope.buildQuery()
                    }
                },
                filter: {
                    and: [
                        {
                            term: { tags: "inbox" }
                        },
                        {
                            range: {
                                "@timestamp": {
                                    "lte": latestTimestamp
                                }
                            }
                        }
                    ]
                }
            }
        },
        size: 1000,
        fields: ["_index", "_type", "_id"],
        sort: [
            {"@timestamp": {order: "desc"}}
        ]
    };

    var archiveEventsByQuery = function () {
        ElasticSearch.search(searchRequest)
            .success(function (response) {
                if (response.hits.hits.length > 0) {
                    ElasticSearch.bulkRemoveTag(response.hits.hits, "inbox")
                        .success(function (response) {
                            $scope.numberArchived += response.items.length;
                            archiveEventsByQuery();
                        })
                        .error(function (error) {
                            console.log("error removing inbox tag:");
                            console.log(error);
                            $scope.error = angular.toJson(angular.fromJson(error), true);
                        });
                }
                else {
                    // Wrapped in timeout so the user can see the modal if
                    // there were very few events to archive.
                    $timeout($scope.$close, 500);
                }
            })
            .error(function (error) {
                console.log("error searching events:");
                console.log(error);
                $scope.error = angular.toJson(angular.fromJson(error), true);
            });
    };

    archiveEventsByQuery();

});