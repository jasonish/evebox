// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
struct ErrorResponse {
    pub code: u16,
    pub error: String,
}
