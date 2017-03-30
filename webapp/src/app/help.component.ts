import {Component, OnInit, OnDestroy} from '@angular/core';
import {AppService, AppEvent, AppEventCode} from './app.service';
import {ApiService} from './api.service';

declare var $: any;

@Component({
    selector: 'evebox-help',
    template: `<div class="modal fade" id="help-modal" tabindex="-1" role="dialog"
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
              <tr *ngFor="let shortcut of shortcuts">

                <td class="col-md-1 evebox-help-table-shortcut">
                  {{shortcut.shortcut}}
                </td>

                <td class="col-md-11">
                  {{shortcut.help}}
                </td>

              </tr>
            </table>
          </div>
          <div
              role="tabpanel"
              class="tab-pane fade" id="help-tab-about">

            <div style="padding: 12px;">

              <p>This is EveBox version {{versionInfo.version}} (Rev: {{versionInfo.revision}}).
              </p>

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
</div>`
})
export class EveboxHelpComponent implements OnInit, OnDestroy {

    shortcuts: any[] = [
        {
            shortcut: '?',
            help: 'Show help.'
        },

        {
            shortcut: 'g i',
            help: 'Goto inbox.'
        },
        {
            shortcut: 'g x',
            help: 'Goto escalated.'
        },
        {
            shortcut: 'g a',
            help: 'Goto alerts.'
        },
        {
            shortcut: 'g e',
            help: 'Goto events.'
        },

        {
            shortcut: 'F8',
            help: 'In inbox, archives active alert.'
        },

        {
            shortcut: 'F9',
            help: 'In inbox, escalate and archive active alert.'
        },

        {
            shortcut: 'e',
            help: 'Archive selected alerts.'
        },
        {
            shortcut: 's',
            help: 'Toggles escalated status of alert.'
        },
        {
            shortcut: 'x',
            help: 'Select highlighted event.'
        },
        {
            shortcut: '/',
            help: 'Focus search input.'
        },
        {
            shortcut: 'j',
            help: 'Next event.'
        },
        {
            shortcut: 'k',
            help: 'Previous event.'
        },
        {
            shortcut: 'o',
            help: 'Open event.'
        },
        {
            shortcut: 'u',
            help: 'When in event view, go back to event listing.'
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
        $('#help-modal').modal();
    }

    eventHandler(appEvent: AppEvent) {
        if (appEvent.event === AppEventCode.SHOW_HELP) {
            this.showHelp();
        }
    }
}