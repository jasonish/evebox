// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { enableProdMode } from "@angular/core";
import { platformBrowserDynamic } from "@angular/platform-browser-dynamic";
import { AppModule } from "./app/app.module";
import { environment } from "./environments/environment";
import { ThemeService } from "./app/shared/theme.service";
import { SettingsService } from "./app/settings.service";
import { initChartjs } from "./app/shared/chartjs";

if (environment.production) {
    console.log("Enabling production mode from ng cli environment.");
    enableProdMode();
}

initChartjs();

new ThemeService(new SettingsService()).init();

platformBrowserDynamic()
    .bootstrapModule(AppModule)
    .catch((err) => console.log(err));
