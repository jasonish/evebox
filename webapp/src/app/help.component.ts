// Copyright (C) 2014-2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE
// OR OTHER DEALINGS IN THE SOFTWARE.

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
