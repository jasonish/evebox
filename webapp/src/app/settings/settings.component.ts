/* Copyright (c) 2017 Jason Ish
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

import { Component, OnInit } from "@angular/core";
import { ThemeService } from "../shared/theme.service";
import {
    SETTING_ALERTS_PER_PAGE,
    SETTING_THEME,
    SettingsService,
} from "../settings.service";

@Component({
    selector: "app-settings",
    templateUrl: "./settings.component.html",
    styleUrls: ["./settings.component.scss"],
})
export class SettingsComponent implements OnInit {
    model = {
        theme: "",
        alertsPerPage: 100,
    };

    constructor(
        private theme: ThemeService,
        private settings: SettingsService
    ) {
        this.model.theme = settings.get(SETTING_THEME, "default");
        this.model.alertsPerPage = settings.getInt(
            SETTING_ALERTS_PER_PAGE,
            100
        );
    }

    ngOnInit() {}

    currentTheme(): string {
        return this.theme.currentTheme();
    }

    setTheme() {
        this.theme.setTheme(this.model.theme);
    }

    updateAlertsPerPage() {
        this.settings.set(SETTING_ALERTS_PER_PAGE, this.model.alertsPerPage);
    }
}
