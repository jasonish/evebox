// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { Alert, Container } from "solid-bootstrap";
import { Top } from "./Top";
import {
  currentThemeName,
  getViewSize,
  setTheme,
  setViewSize,
} from "./settings";
import { getClientPreferences, saveClientPreferences } from "./preferences";

import * as bootstrap from "bootstrap";
import { createEffect } from "solid-js";
import { BiQuestionCircle } from "./icons";

export function Settings() {
  createEffect(() => {
    const popoverTriggerList = document.querySelectorAll(
      '[data-bs-toggle="popover"]',
    );
    [...popoverTriggerList].map(
      (popoverTriggerEl) =>
        new bootstrap.Popover(popoverTriggerEl, { html: true }),
    );
  });

  const query_timeout_tooltip =
    "<b>Experimental:</b> Timeout queries after a number of seconds. 0 to disable timeout. If set, recommended to be at least 3 seconds. Not applied to all queries yet. <br/>Default: 5 (disabled)";

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

        <div class="row mt-2">
          <div class="col"></div>
          <div class="col-sm-12 col-md-8 col-lg-6">
            <div class={"row form-group"}>
              <label class="col-md-4 col-form-label">
                Query Timeout
                <span
                  class="float-end"
                  data-bs-container="body"
                  data-bs-toggle="popover"
                  data-bs-placement="right"
                  data-bs-title="Query Timeout"
                  data-bs-content={query_timeout_tooltip}
                >
                  <BiQuestionCircle />
                </span>
              </label>

              <div class="col-md-8">
                <input
                  class="form-control"
                  type="number"
                  value={getClientPreferences().query_timeout || 0}
                  onChange={(e) => {
                    let value: undefined | number = +e.target.value;
                    if (isNaN(value) || value < 0) {
                      console.log(
                        `Invalid query timeout value: ${e.target.value}, will use default.`,
                      );
                      value = undefined;
                    }
                    console.log(`New query timeout value: ${e.target.value}`);
                    let prefs = getClientPreferences();
                    prefs.query_timeout = value;
                    saveClientPreferences(prefs);
                  }}
                />
              </div>
            </div>
          </div>
          <div class="col"></div>
        </div>
      </Container>
    </>
  );
}
