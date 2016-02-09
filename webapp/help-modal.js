/* Copyright (c) 2015 Jason Ish
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

import * as evebox from "./evebox";

(function () {

    let template = `<div class="modal fade" id="help-modal" tabindex="-1" role="dialog"
     aria-labelledby="help-modal-label">
  <div class="modal-dialog modal-lg" role="document">
    <div class="modal-content">
      <div class="modal-body">

        <ul class="nav nav-tabs" role="tablist">
          <li role="presentation" class="active"><a data-toggle="tab"
                                                    data-target="#help-keyboard-shortcuts">Keyboard
            Shortcuts</a></li>
          <li role="presentation"><a data-toggle="tab"
                                     data-target="#help-tab-about">About</a>
          </li>
        </ul>

        <div class="tab-content">
          <div role="tabpanel" class="tab-pane fade in active"
               id="help-keyboard-shortcuts">
            <table class="table table-bordered evebox-help-table">
              <tr ng-repeat="help in vm.help">

                <td class="col-md-1 evebox-help-table-shortcut">
                  {{help.shortcut}}
                </td>

                <td class="col-md-11">
                  {{help.help}}
                </td>

              </tr>
            </table>
          </div>
          <div
              style="border: 1px solid lightgray !important; border-top: 0 !important;"
              role="tabpanel"
              class="tab-pane fade" id="help-tab-about">

            <div style="padding: 12px;">

              <p>This is EveBox {{vm.EVEBOX_VERSION}}.</p>

              <p>Github:
                <a href="http://github.com/jasonish/evebox">http://github.com/jasonish/evebox</a>
            </div>

          </div>
        </div>

      </div>

      <div class="modal-footer" style="border: 0 !important;">
        <button type="button" class="btn btn-default"
                data-dismiss="modal">Close
        </button>
      </div>

    </div>
  </div>
</div>`;

    angular.module("app").directive("helpModal", helpModal);

    function helpModal() {

        let help = [
            {
                shortcut: "?",
                help: "Show help."
            },

            {
                shortcut: "g i",
                help: "Goto inbox."
            },
            {
                shortcut: "g x",
                help: "Goto escalated."
            },
            {
                shortcut: "g a",
                help: "Goto alerts."
            },
            {
                shortcut: "g e",
                help: "Goto events."
            },

            {
                shortcut: "F8",
                help: "In inbox, archives alerts."
            },

            {
                shortcut: "F9",
                help: "Escalate and archive alerts."
            },

            {
                shortcut: "e",
                help: "Archive selected alerts."
            },
            {
                shortcut: "s",
                help: "Toggles escalated status of alert."
            },
            {
                shortcut: "x",
                help: "Select highlighted event."
            },
            {
                shortcut: "/",
                help: "Focus search input."
            },
            {
                shortcut: "j",
                help: "Next event."
            },
            {
                shortcut: "k",
                help: "Previous event."
            },
            {
                shortcut: "o",
                help: "Open event."
            },
            {
                shortcut: "u",
                help: "When in event view, go back to event listing."
            }
        ];

        return {
            restrict: "AE",
            template: template,
            controller: ["$scope", controller],
            controllerAs: "vm",
            bindToController: true
        };

        function controller($scope) {

            let vm = this;

            vm.help = help;
            vm.EVEBOX_VERSION = evebox.EVEBOX_VERSION;

            $scope.$on("evebox.showHelp", () => {
                $("#help-modal").modal();
            });

        }
    }

})();

