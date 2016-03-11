/* Copyright (c) 2014-2015 Jason Ish
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
 * The events view is a view for all events.
 */

import angular from "angular";
import * as appEvents from "./app-events";

(function () {

    angular.module("app").directive("events", events);

    events.$inject =
        ["$anchorScroll", "$location", "TopNavService", "EventRepository",
            "Keyboard", "StateService"];

    function events($anchorScroll, $location, TopNavService, EventRepository,
                    Keyboard, StateService) {

        return {
            restrict: "AE",
            scope: {},
            template: template,
            controller: ["$scope", controller],
            controllerAs: "ctrl",
            bindToController: true
        };

        function controller($scope) {

            var ctrl = this;

            // Initial state.
            ctrl.loading = false;
            ctrl.activeRow = 0;
            ctrl.queryString = decodeURIComponent($location.search().q || "");
            ctrl.size = 100;
            ctrl.timeStart = $location.search().timeStart;
            ctrl.timeEnd = $location.search().timeEnd;

            //console.log($location.search().from);

            let stateKey = $location.$$url;

            ctrl.openEvent = function (event) {
                if (Number(event) == event) {
                    event = ctrl.events[ctrl.activeRow];
                }
                $location.path(`event/${event._id}`);
            };

            ctrl.gotoPrevPage = () => {
                let prev = $location.search().timeStart;
                let current = ctrl.resultSet.latest;
                $location.search("timeStart", ctrl.resultSet.latest);
                $location.search("timeEnd", "");
                if (prev == current) {
                    refresh();
                }
            };

            ctrl.gotoNextPage = () => {
                $location.search("timeEnd", ctrl.resultSet.earliest);
                $location.search("timeStart", "");
                //refresh();
            };

            Keyboard.bind($scope, "/", (e) => {
                e.preventDefault();
                $anchorScroll();
                document.getElementById("query-string-input").focus();
            });

            $scope.$on("$stateChangeStart", () => {
                StateService.put(stateKey, {
                    activeRow: ctrl.activeRow,
                    events: ctrl.events
                });
            });

            ctrl.refresh = () => {
                $location.search("timeStart", undefined);
                $location.search("timeEnd", undefined);
                refresh();
            };

            $scope.$on(appEvents.TIMERANGE_CHANGED, refresh);

            function refresh() {


                if (ctrl.queryString) {
                    $location.search("q", ctrl.queryString);
                }
                $(".form-control").blur();
                EventRepository.searchEvents({
                    queryString: ctrl.queryString,
                    timeRange: TopNavService.timeRange,
                    timeEnd: $location.search().timeEnd,
                    timeStart: $location.search().timeStart
                }).then(response => {
                    ctrl.activeRow = 0;
                    ctrl.events = response.results;
                    ctrl.resultSet = response;
                    console.log(response);
                })
            }

            if (StateService.get(stateKey)) {
                let state = StateService.get(stateKey);
                ctrl.activeRow = state.activeRow;
                ctrl.events = state.events;
            }
            else {
                refresh();
            }

        }

    }

    let template = `<div class="row">
  <div class="col-md-12">
    <form ng-submit="ctrl.refresh()">
      <fieldset ng-disabled="ctrl.loading">
        <div class="form-group" style="margin-bottom: 0px;">
          <div class="input-group">
            <input id="query-string-input" type="text" class="form-control"
                   ng-model="ctrl.queryString"/>
      <span class="input-group-btn">
        <button type="submit" class="btn btn-default">Search</button>
      </span>
          </div>
        </div>
      </fieldset>
    </form>
  </div>
</div>

<br/>

<div class="row">
  <div class="col-md-12">

    <button type="button" class="btn btn-default" ng-click="ctrl.refresh()">
      Refresh
    </button>

    <div class="pull-right">
      <button type="button" class="btn btn-default"
              ng-click="ctrl.gotoPrevPage()">
        Newer
      </button>
      <button type="button" class="btn btn-default"
              ng-click="ctrl.gotoNextPage()">
        Older
      </button>
    </div>

  </div>
</div>

<br/>

<div class="row">
  <div class="col-md-12">

    <div ng-if="ctrl.events && ctrl.events.length > 0">

      <div class="table-responsive">
        <table class="table table-condensed app-event-table"
               keyboard-table
               active-row="ctrl.activeRow"
               rows="ctrl.events"
               on-row-open="ctrl.openEvent">
          <thead>
          <tr>
            <th></th>
            <th>Timestamp</th>
            <th>Type</th>
            <th>Source/Dest</th>
            <th>Description</th>
          </tr>
          </thead>
          <tbody>
          <tr ng-repeat="event in ctrl.events" ng-click="ctrl.openEvent(event)"
              ng-class="event | eventSeverityToBootstrapClass">
            <td><span ng-hide="$index != ctrl.activeRow"
                      class="glyphicon glyphicon-chevron-right"></span></td>

            <td class="text-nowrap">
              {{::event._source.timestamp | formatTimestamp}}
              <br/>
              <elapsed-time timestamp="event._source.timestamp"
                            style="color: grey"></elapsed-time>
            </td>
            <td>{{::event._source.event_type | uppercase}}</td>

            <td class="text-nowrap">
              <label>S:</label> {{::event._source.src_ip | formatIpAddress}}
              <br/>
              <label>D:</label> {{::event._source.dest_ip | formatIpAddress}}
            </td>

            <td style="word-break: break-all;">{{::event |
              formatEventDescription}}
            </td>
          </tr>
          </tbody>
        </table>
      </div>

    </div>

  </div>
</div>
`;


})();