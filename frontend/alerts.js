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

import angular from "angular";
import moment from "moment";

import * as evebox from "./evebox";
import * as appEvents from "./app-events";

(function () {

    angular.module("app").directive("alerts", alerts);

    alerts.$inject =
        ["EventRepository", "Keyboard", "StateService", "TopNavService"];

    function alerts(EventRepository, Keyboard, StateService,
                    TopNavService) {

        return {
            restrict: "AE",
            template: template,
            scope: {},
            controller: ["$scope", "$location", "$state", controller],
            controllerAs: "vm",
            bindToController: true
        };

        function controller($scope, $location, $state) {

            let vm = this;

            let filters = [];

            vm.view = $state.current.data.mode;

            switch (vm.view) {
                case "inbox":
                    filters.push({not: {term: {tags: "archived"}}});
                    break;
                case "escalated":
                    filters.push({term: {tags: "escalated"}});
                    TopNavService.timeRangeEnabled = false;
                    break;
                default:
                    break;
            }

            vm.activeItem = 0;
            vm.loading = true;
            vm.filterText = $location.search().q || "";

            vm.onSwipeLeft = (event) => {
                event.showActions = true;
            };

            vm.onSwipeRight = (event) => {
                event.showActions = false;
            };

            vm.isSmallDisplay = () => {
                return !jQuery("#sm").is(":visible");
            };

            vm.hasEvents = () => {
                return vm.events && vm.events.length > 0;
            };

            vm.hasTag = evebox.hasTag;

            vm.isArchived = (event) => {
                return evebox.hasTag(event, "archived");
            };

            /**
             * Select all events, or if they are already all selected, deselect
             * them all.
             */
            vm.selectAll = () => {
                let allSelected = vm.events.every(event => {
                    return event.selected;
                });
                if (allSelected) {
                    vm.events.forEach(event => {
                        event.selected = false;
                    });
                }
                else {
                    vm.events.forEach(event => {
                        event.selected = true;
                    })
                }
            };

            vm.selectBySeverity = (severity) => {
                vm.events.forEach(event => {
                    if (event.newest._source.alert.severity == severity) {
                        event.selected = true;
                    }
                })
            };

            function getSelectedEvents() {
                return vm.events.filter(event => {
                    return event.selected === true;
                });
            }

            vm.refresh = refresh;

            function refresh() {
                vm.loading = true;
                let options = {
                    timeRange: TopNavService.timeRange,
                    queryString: vm.filterText,
                    filters: filters
                };

                if (vm.view != "escalated") {
                    options.timeRange = TopNavService.timeRange;
                }

                EventRepository.getEventsGroupedBySignatureSourceDest(options)
                    .then(response => {
                        vm.activeItem = 0;
                        vm.events = response;
                        vm.loading = false;
                    });
            }

            let stateKey = $location.$$url;

            vm.openEvent = function (event) {
                vm.activeItem = vm.events.indexOf(event);
                StateService.put(stateKey, {
                    activeItem: vm.activeItem,
                    events: vm.events
                });
                StateService.put(event.newest._id, event);
                $state.go("event", {id: event.newest._id});
            };

            function getActiveEvent() {
                return vm.events[vm.activeItem];
            }

            function removeEvent(event) {
                let idx = vm.events.indexOf(event);

                console.log("removeEvent:");
                console.log(idx);

                if (idx < 0) {
                    return;
                }

                if (vm.activeItem > 0 && idx == vm.events.length - 1) {
                    vm.activeItem--;
                }
                else if (vm.activeItem > idx) {
                    vm.activeItem--;
                }

                vm.events.splice(idx, 1);
            }

            vm.toggleEscalated = ($event, event) => {
                if ($event) {
                    $event.stopPropagation();
                }

                EventRepository.toggleEscalated(event).then(() => {
                    if (event.escalated == event.count) {
                        event.escalated = 0;
                    }
                    else {
                        event.escalated = event.count;
                    }
                });
            };

            vm.escalate = (event) => {

                // Can't escalate an event in the escalated view.
                if (vm.view === "escalated") {
                    return;
                }

                // If an event has been passed, escalate it and it only.
                if (event) {

                    // Be optimistic, remove the event from view first.
                    removeEvent(event);

                    EventRepository.addEscalated(event).then(() => {
                        event.escalated = event.count;
                        vm.archiveEvent(event);
                    });
                    return;
                }

                // Next check if we are escalating a group of events.
                let selectedEvents = getSelectedEvents();
                if (selectedEvents.length > 0) {
                    selectedEvents.forEach(vm.escalate);
                    return;
                }

                // Otherwise escalate the current highlighted event.
                vm.escalate(getActiveEvent());
            };

            vm.onFilterSubmit = () => {
                jQuery("#evebox-inbox-filter-form input").blur();
                $location.search("q", vm.filterText);
                refresh();
            };

            function doArchiveEvent(event) {
                EventRepository.submitArchiveEvent(event);

                // Only remove if in inbox.
                if (vm.view == "inbox") {
                    removeEvent(event);
                    return;
                }

                evebox.addTag(event, "archived");
            }

            function archiveForInbox(event) {
                let selectedEvents = getSelectedEvents();
                if (selectedEvents.length) {
                    selectedEvents.forEach(event => {
                        doArchiveEvent(event);
                    });
                }
                else {
                    event = event ? event : getActiveEvent();
                    if (!event) {
                        return;
                    }
                    doArchiveEvent(event);
                }
            }

            function archiveForEscalated(event) {
                if (event) {

                    // Be optimistic and remove the event from view.
                    removeEvent(event);

                    return EventRepository.submit(() => {
                        console.log("Removing star.");
                        return EventRepository.removeEscalated(event).then(() => {
                            return EventRepository.archiveEvent(event);
                        })
                    })

                }

                let selectedEvents = getSelectedEvents();
                if (selectedEvents.length) {
                    selectedEvents.forEach(event => {
                        return archiveForEscalated(event);
                    });
                }
                else {
                    event = event ? event : getActiveEvent();
                    if (event) {
                        return archiveForEscalated(event);
                    }
                }
            }

            vm.archiveEvent = function (event) {
                if (vm.view == "escalated") {
                    return archiveForEscalated(event);
                }
                else {
                    return archiveForInbox(event);
                }
            };

            Keyboard.bind($scope, "e", () => {
                vm.archiveEvent(null, getActiveEvent());
            });

            Keyboard.bind($scope, "f8", () => {
                vm.archiveEvent(null, getActiveEvent());
            });

            Keyboard.bind($scope, "s", () => {
                vm.toggleEscalated(undefined, getActiveEvent());
            });

            Keyboard.bind($scope, "f9", () => {
                vm.escalate();
            });

            Keyboard.bind($scope, "x", () => {
                getActiveEvent().selected = !getActiveEvent().selected;
            });

            Keyboard.bind($scope, "* a", () => {
                vm.selectAll();
            });

            Keyboard.bind($scope, "* 1", () => {
                vm.selectBySeverity(1);
            });

            Keyboard.bind($scope, "* 2", () => {
                vm.selectBySeverity(2);
            });

            Keyboard.bind($scope, "* 3", () => {
                vm.selectBySeverity(3);
            });

            Keyboard.bind($scope, "r", () => {
                vm.refresh();
            });

            Keyboard.bind($scope, "o", () => {
                vm.openEvent(getActiveEvent());
            });

            Keyboard.bind($scope, "/", (e) => {
                e.preventDefault();
                document.getElementById("filter-text-input").focus();
            });

            $scope.$on(appEvents.TIMERANGE_CHANGED, refresh);

            $scope.$on(appEvents.WINDOW_RESIZE, () => $scope.$apply());

            function init() {
                let state = StateService.get(stateKey);
                if (state) {
                    vm.activeItem = state.activeItem;
                    vm.loading = false;

                    if (vm.view == "inbox") {
                        vm.events = _.filter(state.events, (event) => {
                            return !evebox.hasTag(event, "archived");
                        });
                    }
                    else {
                        vm.events = state.events;
                    }

                    if (vm.activeItem >= vm.events.length) {
                        vm.activeItem = vm.events.length - 1;
                    }
                }
                else {
                    refresh();
                }
            }

            init();
        }

    }

    /* Template for small screens (mobile). */
    let smallTemplate = `<ul class="list-group" style="margin-left: 0; margin-right: 0; width: 100%;">
  <li class="list-group-item" ng-repeat="event in vm.events"
      ng-class="event.newest._source.alert.severity | eventSeverityToBootstrapClass: 'list-group-item-'"
      ng-click="vm.openEvent(event)"
      ng-swipe-left="vm.onSwipeLeft(event)"
      ng-swipe-right="vm.onSwipeRight(event)">

    <div ng-if="event.showActions" style="position: absolute; right: 5px; top: 25%; bottom: 25%;">
      <button type="button" class="btn btn-primary"
              ng-click="vm.archiveEvent($event, event);">Archive
      </button>
    </div>

    <span class="badge">{{event.count}}</span>
    {{event.newest._source.alert.signature}}
    <br/>
    {{event.newest._source.src_ip | formatIpAddress}} ->
    {{event.newest._source.dest_ip | formatIpAddress}}
    <br/>
    <elapsed-time timestamp="event.newest._source.timestamp"></elapsed-time>
  </li>
</ul>`;

    /* Template for larger screens. */
    let bigTemplate = `<div class="row">
  <div class="col-md-6">

    <button type="button" class="btn btn-default btn-sm"
            ng-click="vm.refresh()">
      Refresh
    </button>

    <button ng-if="vm.hasEvents()" type="button" class="btn btn-default btn-sm"
            ng-click="vm.selectAll()">
      Select All
    </button>

    <button ng-if="vm.hasEvents()" type="button" class="btn btn-default btn-sm"
            ng-click="vm.archiveEvent()">
      Archive
    </button>

    <button ng-if="vm.view != 'escalated' && vm.hasEvents()"
            type="button" class="btn btn-default btn-sm"
            ng-click="vm.escalate()">
      Escalate
    </button>

  </div>
  <div class="col-md-6">
    <form id="evebox-inbox-filter-form" ng-show="vm.events"
          ng-submit="vm.onFilterSubmit()">
      <div class="input-group">
        <input type="text" id="filter-text-input" class="form-control"
               ng-model="vm.filterText"
               placeholder="Filter..."/>
      <span class="input-group-btn">
        <button class="btn btn-default" type="submit">Apply</button>
      </span>
      </div>
    </form>

  </div>
</div>

<div ng-if="!vm.loading && !vm.hasEvents()" style="text-align: center;">
  <hr/>
  No new events.
  <hr/>
</div>

<div ng-if="vm.loading" class="row">
  <div class="col-md-12">
    <i class="fa fa-spinner fa-pulse"
       style="font-size: 300px; position: absolute; left: 50%; margin-left: -150px; opacity: 0.8;"></i>
  </div>
</div>

<div class="table-responsive" ng-if="vm.hasEvents()">
  <br/>
  <table class="table table-condensed table-hover app-event-table"
         keyboard-table
         active-row="vm.activeItem"
         nf-if="vm.events.length > 0"
         rows="vm.events">
    <thead>
    <th></th>
    <th></th>
    <th></th>
    <th>#</th>
    <th>Timestamp</th>
    <th>Source/Dest</th>
    <th width="50%;">Signature</th>
    </thead>
    <tbody>
    <tr ng-repeat="event in vm.events" ng-click="vm.openEvent(event)"
        ng-class="event.newest._source.alert.severity | eventSeverityToBootstrapClass">
      <td><span ng-hide="$index != vm.activeItem"
                class="glyphicon glyphicon-chevron-right"></span></td>
      <td ng-click="$event.stopPropagation()">
        <input type="checkbox" style="margin-top: 2px;"
               ng-model="event.selected"
               ng-click="$event.stopPropagation"/>
      </td>
      <td ng-click="vm.toggleEscalated($event, event)">
        <i ng-if="event.escalated == 0" class="fa fa-star-o"></i>
        <i ng-if="event.escalated == event.count" class="fa fa-star"></i>
        <i ng-if="event.escalated > 0 && event.escalated != event.count"
           class="fa fa-star-half-o"></i>
      </td>
      <td>{{::event.count}}</td>
      <td class="text-nowrap">
        {{::event.newest._source.timestamp | formatTimestamp}}
        <br/>
        <elapsed-time timestamp="event.newest._source.timestamp"
                      style="color: grey"></elapsed-time>
      </td>
      <td class="text-nowrap">
        <label>S:</label>
        {{::event.newest._source.src_ip | formatIpAddress}}
        <br/>
        <label>D:</label>
        {{::event.newest._source.dest_ip | formatIpAddress}}
      </td>
      <td>
        <span class="pull-right" style="float: right;"
              ng-if="!vm.isArchived(event)"><button
            class="btn btn-default"
            ng-click="$event.stopPropagation(); vm.archiveEvent(event);">Archive
        </button></span>
        {{::event.newest._source.alert.signature}}
      </td>
    </tr>
    </tbody>
  </table>
</div>`;

    /* The final template. */
    let template = `<div ng-class="{'opacity-50': vm.loading}">
  <div ng-if="vm.isSmallDisplay()">
    ${smallTemplate}
  </div>
  <div ng-if="!vm.isSmallDisplay()">
    ${bigTemplate}
  </div>
</div>`;

})();

