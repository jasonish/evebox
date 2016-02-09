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
 * The view for changing settings.
 */

import angular from "angular";
import lodash from "lodash";

(function() {

    angular.module("app").directive("settings", settings);

    settings.$inject = ["Config"];

    function settings(Config) {

        let template = `<form name="settings">
  <div class="form-group">
    <label>Elastic Search URL</label>
    <input type="text" class="form-control"
           ng-model="vm.config.elasticSearch.url"
           placeholder="{{vm.placeholder}}"/>
  </div>
  <div class="row">
    <div class="col-md-4">
      <button type="submit" class="btn btn-primary btn-block"
              ng-disabled="!settings.$dirty" ng-click="vm.save()">Save
      </button>
      <br/>
    </div>
    <div class="col-md-4">
      <button type="submit" class="btn btn-default btn-block"
              ng-disabled="!settings.$dirty" ng-click="vm.discardChanges()">Discard Changes
      </button>
      <br/>
    </div>
    <div class="col-md-4">
      <button type="submit" class="btn btn-warning btn-block" ng-click="vm.resetToDefaults()">Reset to
        Defaults
      </button>
    </div>
  </div>
</form>
    `;

        function controller() {

            window.Config = Config;

            var vm = this;

            vm.config = lodash.cloneDeep(Config.getConfig());
            vm.placeholder = `${window.location.protocol}://${window.location.hostname}:9200`;

            vm.save = function() {
                Config.getConfig().elasticSearch = vm.config.elasticSearch;
                Config.save();
                location.reload();
            };

            vm.discardChanges = function() {
                vm.config = lodash.cloneDeep(Config.getConfig());
            };

            vm.resetToDefaults = function() {
                Config.resetToDefaults();
                vm.config = lodash.cloneDeep(Config.getConfig());
                location.reload();
            }
        }

        return {
            restrict: "AE",
            scope: {},
            template: template,
            controller: controller,
            controllerAs: "vm",
            bindToController: true
        };

    }

})();

