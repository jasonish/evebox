// Copyright (C) 2014-2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

import {Component, OnDestroy, OnInit} from "@angular/core";
import {AppEvent, AppEventCode, AppService} from "./app.service";
import {ApiService} from "./api.service";

declare var $: any;

@Component({
    selector: "evebox-help",
    template: `
      <div class="modal fade" id="help-modal" tabindex="-1" role="dialog"
           aria-labelledby="help-modal-label">
        <div class="modal-dialog modal-lg" role="document">
          <div class="modal-content">
            <div class="modal-body">

              <ul class="nav nav-tabs" role="tablist">
                <li class="nav-item">
                  <a data-toggle="tab" class="nav-link active"
                     href="#"
                     data-target="#help-keyboard-shortcuts">Keyboard
                    Shortcuts</a></li>
                <li class="nav-item">
                  <a data-toggle="tab" class="nav-link"
                     href="#"
                     data-target="#help-tab-about">About</a>
                </li>
              </ul>

              <div class="tab-content">

                <div class="tab-pane fade show active"
                     id="help-keyboard-shortcuts">
                  <table class="table table-bordered evebox-help-table table-sm">
                    <tr *ngFor="let shortcut of shortcuts">

                      <td class="evebox-help-table-shortcut">
                        {{shortcut.shortcut}}
                      </td>

                      <td>
                        {{shortcut.help}}
                      </td>

                    </tr>
                  </table>
                </div>

                <div class="tab-pane fade" id="help-tab-about">

                    <app-about></app-about>

                </div>
              </div>

            </div>

            <div class="modal-footer" style="border: 0 !important;">
              <button type="button" class="btn btn-secondary"
                      data-dismiss="modal">Close
              </button>
            </div>

          </div>
        </div>
      </div>`
})
export class EveboxHelpComponent implements OnInit, OnDestroy {

    shortcuts: any[] = [
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
            help: "In inbox, archives active alert."
        },

        {
            shortcut: "F9",
            help: "In inbox, escalate and archive active alert."
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
        },
        {
            shortcut: "* a",
            help: "Select all alerts in view.",
        },
        {
            shortcut: "* n",
            help: "Deselect all alerts.",
        },
        {
            shortcut: "* 1",
            help: "Select all alerts with same SID as current alert.",
        },
        {
            shortcut: ".",
            help: "Dropdown alert menu.",
        },
    ];

    appServiceSubscription: any;

    constructor(private appService: AppService, private api: ApiService) {
    }

    ngOnInit() {
        this.appServiceSubscription = this.appService.subscribe(
            (event: AppEvent) => this.eventHandler(event));
    }

    ngOnDestroy() {
        this.appServiceSubscription.unsubscribe();
    }

    showHelp() {
        $("#help-modal").modal();
    }

    eventHandler(appEvent: AppEvent) {
        if (appEvent.event === AppEventCode.SHOW_HELP) {
            this.showHelp();
        }
    }
}
