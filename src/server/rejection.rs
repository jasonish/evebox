// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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
