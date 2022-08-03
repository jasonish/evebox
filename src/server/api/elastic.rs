// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use axum::response::IntoResponse;
use hyper::StatusCode;

pub(crate) async fn get_index() -> impl IntoResponse {
    StatusCode::OK
}
