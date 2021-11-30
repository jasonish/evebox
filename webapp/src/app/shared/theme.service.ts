/* Copyright (c) 2014-2021 Jason Ish
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

import { Injectable } from '@angular/core';
import { SETTING_THEME, SettingsService } from '../settings.service';

declare var Chart: any;

@Injectable()
export class ThemeService {

    constructor(private settings?: SettingsService) {
    }

    init(): void {
        this.setTheme(this.currentTheme());
    }

    currentTheme(): string {
        let theme = this.settings.get(SETTING_THEME);
        if (!theme) {
            theme = "default";
        }
        return theme;
    }

    setTheme(theme: string): void {
        switch (theme) {
            case "dark":
                document.getElementsByTagName("html")[0].setAttribute("class", "dark dark-mode");
                Chart.defaults.global.defaultFontColor = "#fff";
                break;
            default:
                document.getElementsByTagName("html")[0].setAttribute("class", "light");
                Chart.defaults.global.defaultFontColor = "#666";
                break;
        }
        this.settings.set(SETTING_THEME, theme);
    }
}
