/* Copyright (c) 2014-2016 Jason Ish
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

                  <div style="padding: 12px;">

                    <p>This is EveBox version {{versionInfo.version}} (Rev: {{versionInfo.revision}}).
                    </p>

                    <p>Homepage: <a href="https://evebox.org">https://evebox.org</a></p>

                    <p>GitHub:
                      <a href="http://github.com/jasonish/evebox">http://github.com/jasonish/evebox</a>
                  </div>

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
        }
    ];

    versionInfo: any = {};

    appServiceSubscription: any;

    constructor(private appService: AppService, private api: ApiService) {
    }

    ngOnInit() {
        this.appServiceSubscription = this.appService.subscribe(
            (event: AppEvent) => this.eventHandler(event));

        this.api.getVersion().then((response) => {
            this.versionInfo = response;
        });
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
