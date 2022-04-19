// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import {Injectable} from "@angular/core";
import {SETTING_THEME, SettingsService} from "../settings.service";
import {Chart} from "chart.js";

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

    loadTheme(name: string): void {
        let html = document.getElementsByTagName("html")[0];
        html.setAttribute("class", "invisible");

        document.getElementById("theme")?.remove();
        let e = document.createElement("link");
        e.rel = "stylesheet";
        e.type = "text/css";
        e.id = "theme";
        e.media = "all";
        e.href = name;
        document.body.appendChild(e);

        // The prevents the unstyled page from briefly showing through.
        setTimeout(() => {
            html.removeAttribute("class");
        }, 0);
    }

    setTheme(theme: string): void {
        switch (theme) {
            case "dark":
                this.loadTheme("dark.css");
                Chart.defaults.color = "#fff";
                break;
            default:
                this.loadTheme("light.css");
                Chart.defaults.color = "#666";
                break;
        }
        this.settings.set(SETTING_THEME, theme);
    }
}
