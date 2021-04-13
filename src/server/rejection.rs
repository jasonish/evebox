// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::convert::Infallible;

use serde_json::json;
use warp::reply::Reply;

use crate::datastore::DatastoreError;
use crate::logger::log;
use crate::server::api;
use crate::server::response;

pub async fn rejection_handler(err: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    // Look for API errors.
    if let Some(err) = err.find::<api::ApiError>() {
        return Ok(response::Response::build_error_response(
            warp::http::StatusCode::BAD_REQUEST,
            &err.to_string(),
        ));
    }

    if let Some(err) = err.find::<DatastoreError>() {
        return Ok(response::Response::build_error_response(
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            &format!("datastore error: {}", err),
        ));
    }

    if let Some(err) = err.find::<warp::reject::PayloadTooLarge>() {
        dbg!(err);
        return Ok(response::Response::build_error_response(
            warp::http::StatusCode::BAD_REQUEST,
            "payload too large",
        ));
    }

    if let Some(super::main::GenericError::AuthenticationRequired) = err.find() {
        let code = warp::http::StatusCode::UNAUTHORIZED;
        let response = json!({
            "status": code.as_u16(),
            "error": "authentication required",
        });
        return Ok(warp::reply::with_status(warp::reply::json(&response), code).into_response());
    }

    // Look for our own not found error.
    if let Some(super::main::GenericError::NotFound) = err.find() {
        println!("found not found error");
        return Ok(response::Response::build_error_response(
            warp::http::StatusCode::NOT_FOUND,
            "not found",
        ));
    }

    if err.is_not_found() {
        return Ok(response::Response::build_error_response(
            warp::http::StatusCode::NOT_FOUND,
            "not found",
        ));
    }

    log::warn!(
        "Unhandled rejection, returning internal server error: {:?}",
        err
    );

    Ok(warp::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
