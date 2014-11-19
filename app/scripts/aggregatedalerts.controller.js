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

    angular.module("app")
        .controller("AggregatedAlertsController", AggregatedAlertsController);

    function AggregatedAlertsController($scope,
        $location,
        Keyboard, ElasticSearch, $modal, $routeParams,
        NotificationMessageService,
        Util) {

        /**
         * Opens an aggregation in "flat" view by explicitly setting no aggregation
         * and constructing a query.
         */
        $scope.openAggregation = function (agg) {
            var searchParams = $location.search();
            searchParams.aggregateBy = "";
            searchParams.q = "q" in searchParams ? searchParams.q : "";
            searchParams.q += " +alert.signature.raw:\"" + agg.signature + "\"";
            if (agg.src_ip) {
                searchParams.q += " +src_ip.raw:\"" + agg.src_ip + "\"";
            }
            searchParams.q = searchParams.q.trim();

            $location.search(searchParams);
        };

        $scope.totalEventCount = function () {
            return _.reduce($scope.aggregations, function (sum, agg) {
                return sum + agg.count;
            }, 0);
        };

        $scope.deleteRow = function (row) {
            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        }
                    }
                }
            };

            query.query.filtered.filter = {
                "and": _.cloneDeep($scope.filters)
            };

            query.query.filtered.filter.and.push({
                "range": {
                    "@timestamp": {
                        "lte": row.last_timestamp
                    }
                }
            });

            query.query.filtered.filter.and.push({
                "term": {
                    "alert.signature.raw": row.signature
                }
            });

            if (row.src_ip) {
                query.query.filtered.filter.and.push({
                    "term": {
                        "src_ip.raw": row.src_ip
                    }
                });
            }

            return ElasticSearch.deleteByQuery(query);

        };

        $scope.deleteSelected = function () {
            var selectedRows = $scope.getSelectedRows();

            if (!selectedRows) {
                return;
            }

            var row = selectedRows.pop();

            $scope.deleteRow(row)
                .success(function (response) {
                    $scope.removeRow(row);
                    NotificationMessageService.add(
                        "info",
                        Util.formatString("Deleted: {0}", row.signature));
                })
                .finally(function () {
                    $scope.deleteSelected();
                });
        };

        $scope.buildArchiveRowQuery = function (row) {
            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        },
                        filter: {
                            and: [
                                {
                                    term: {tags: "inbox"}
                                },
                                {
                                    range: {
                                        "@timestamp": {
                                            "lte": row.last_timestamp
                                        }
                                    }
                                },
                                {
                                    term: {
                                        "alert.signature.raw": row.signature
                                    }
                                }
                            ]
                        }
                    }
                },
                size: 1000,
                fields: []
            };

            if (row.src_ip) {
                query.query.filtered.filter.and.push({
                    "term": {
                        "src_ip.raw": row.src_ip
                    }
                });
            }

            return query;
        };

        $scope.archiveSelected = function () {

            if ($scope.$routeParams.view != "inbox") {
                return NotificationMessageService.add("warning", "Archive not valid in this context.");
            }

            var selectedRows = $scope.getSelectedRows();

            if (selectedRows.length == 0) {
                return NotificationMessageService.add("warning", "No events selected.");
            }

            var archiveJobs = selectedRows.map(function (row) {
                return {
                    row: row,
                    label: row.signature,
                    query: $scope.buildArchiveRowQuery(row)
                };
            });

            var modal = $modal.open({
                templateUrl: "templates/modal-progress.html",
                controller: "ModalProgressController",
                resolve: {
                    jobs: function () {
                        return archiveJobs;
                    }
                }
            });

            modal.result.then(function () {
                if ($scope.aggregations.length == 0) {
                    $scope.refresh();
                }
            });

            var doArchiveJob = function () {

                if (archiveJobs.length == 0) {
                    return;
                }

                var job = archiveJobs[0];

                ElasticSearch.search(job.query)
                    .success(function (response) {
                        if (job.max === undefined) {
                            job.max = response.hits.total;
                            job.value = 0;
                        }
                        if (response.hits.hits.length > 0) {
                            ElasticSearch.bulkRemoveTag(response.hits.hits, "inbox")
                                .success(function (response) {
                                    job.value += response.items.length;
                                    doArchiveJob();
                                });
                        }
                        else {
                            $scope.removeRow(job.row);
                            archiveJobs.shift();
                            if (archiveJobs.length == 0) {
                                console.log("No archive jobs left; closing modal.");
                                modal.close();
                            }
                            else {
                                doArchiveJob();
                            }
                        }
                    });

            };

            doArchiveJob();

        };

        $scope.archiveByQuery = function () {
            if ($scope.response.hits.total == 0) {
                NotificationMessageService.add("warning", "No events to archive.");
                return;
            }

            var lastTimestamp = _.max($scope.aggregations, function (row) {
                return row.last_timestamp;
            }).last_timestamp;

            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        },
                        filter: {
                            and: [
                                {
                                    term: {tags: "inbox"}
                                },
                                {
                                    range: {
                                        "@timestamp": {
                                            "lte": lastTimestamp
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

            $scope.doArchiveByQuery("Archiving...", query);
        };

        $scope.deleteByQuery = function () {
            if ($scope.response.hits.total == 0) {
                NotificationMessageService.add("warning", "No events to delete.");
                return;
            }

            var lastTimestamp = _.max($scope.aggregations, function (row) {
                return row.last_timestamp;
            }).last_timestamp;

            var query = {
                query: {
                    filtered: {
                        query: {
                            query_string: {
                                query: $scope.searchModel.userQuery || "*"
                            }
                        },
                        filter: {
                            and: [
                                {
                                    range: {
                                        "@timestamp": {
                                            "lte": lastTimestamp
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            };

            _.forEach($scope.filters, function (filter) {
                query.query.filtered.filter.and.push(filter);
            });

            ElasticSearch.deleteByQuery(query)
                .success(function (response) {
                    NotificationMessageService.add("success", "Events deleted.");
                    $scope.page = 1;
                    $scope.refresh();
                })
                .error(function (error) {
                    console.log(error);
                })
        };

        $scope.changeSortBy = function (what) {
            if ($scope.searchModel.sortBy == what) {
                $location.search("sortByOrder",
                    $scope.searchModel.sortByOrder == "desc" ? "asc" : "desc");
            }
            else {
                $location.search("sortBy", what);
            }
        };

        $scope.$on("$destroy", function () {
            Keyboard.resetScope($scope);
        });

        Keyboard.scopeBind($scope, "o", function () {
            $scope.$apply(function () {
                var row = $scope.getActiveRow();
                $scope.openAggregation(row);
            });
        });

        Keyboard.scopeBind($scope, "shift+j", function () {
            $scope.toggleActiveRowCheckbox();
            Mousetrap.trigger("j");
        });

        Keyboard.scopeBind($scope, "shift+k", function () {
            $scope.toggleActiveRowCheckbox();
            Mousetrap.trigger("k");
        });

        Keyboard.scopeBind($scope, "e", function () {
            $scope.$apply(function () {
                $scope.archiveSelected();
            });
        });

        Keyboard.scopeBind($scope, "#", function () {
            $scope.$apply(function () {
                $scope.deleteSelected();
            });
        });
    };

})();

