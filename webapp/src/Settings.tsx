// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Alert, Col, Container, Row } from "solid-bootstrap";
import { Top } from "./Top";
import {
  currentThemeName,
  getViewSize,
  setTheme,
  setViewSize,
} from "./settings";
import { getClientPreferences, saveClientPreferences } from "./preferences";

export function Settings() {
  return (
    <>
      <Top />
      <Container class={"mt-2"}>
        <div class="row">
          <div class="col"></div>
          <div class="col-sm-12 col-md-8 col-lg-6">
            <Alert variant={"info"}>
              Note: Settings are stored client side and will not be reflected on
              other computers or in other browsers.
            </Alert>
          </div>
          <div class="col"></div>
        </div>

        <div class="row">
          <div class="col"></div>
          <div class="col-sm-12 col-md-8 col-lg-6">
            <div class={"row form-group"}>
              <label class="col-md-4 col-form-label">Theme</label>
              <div class="col-md-8">
                <select
                  class="form-select"
                  onchange={(e) => setTheme(e.currentTarget.value)}
                >
                  <option
                    value="light"
                    selected={currentThemeName() === "light"}
                  >
                    Light
                  </option>
                  <option value="dark" selected={currentThemeName() === "dark"}>
                    Dark
                  </option>
                </select>
              </div>
            </div>
          </div>
          <div class="col"></div>
        </div>

        <div class="row mt-2">
          <div class="col"></div>
          <div class="col-sm-12 col-md-8 col-lg-6">
            <div class={"row form-group"}>
              <label class="col-md-4 col-form-label">View Size</label>
              <div class="col-md-8">
                <select
                  class="form-select"
                  onchange={(e) => setViewSize(e.currentTarget.value)}
                >
                  <option value="100" selected={getViewSize() === 100}>
                    100
                  </option>
                  <option value="200" selected={getViewSize() === 200}>
                    200
                  </option>
                  <option value="300" selected={getViewSize() === 300}>
                    300
                  </option>
                  <option value="400" selected={getViewSize() === 400}>
                    400
                  </option>
                  <option value="500" selected={getViewSize() === 500}>
                    500
                  </option>
                  <option value="fit" selected={getViewSize() === "fit"}>
                    Fit to Height
                  </option>
                </select>
              </div>
            </div>
          </div>
          <div class="col"></div>
        </div>

        <div class="row mt-2">
          <div class="col"></div>
          <div class="col-sm-12 col-md-8 col-lg-6">
            <div class={"row form-group"}>
              <label class="col-md-4 col-form-label">Timestamp Format</label>
              <div class="col-md-8">
                <select
                  class="form-select"
                  onchange={(e) => {
                    let prefs = getClientPreferences();
                    switch (e.currentTarget.value) {
                      case "utc":
                        prefs.timestamp_format = "utc";
                        break;
                      default:
                        prefs.timestamp_format = undefined;
                        break;
                    }
                    saveClientPreferences(prefs);
                  }}
                >
                  <option
                    value="local"
                    selected={
                      getClientPreferences().timestamp_format === "local"
                    }
                  >
                    Local
                  </option>
                  <option
                    value="utc"
                    selected={getClientPreferences().timestamp_format === "utc"}
                  >
                    UTC
                  </option>
                </select>
              </div>
            </div>
          </div>
          <div class="col"></div>
        </div>
      </Container>
    </>
  );
}
