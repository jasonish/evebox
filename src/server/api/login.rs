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

use crate::prelude::*;
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use crate::server::main::SessionExtractor;
use crate::server::session::Session;
use crate::server::AuthenticationType;
use crate::server::ServerContext;
use crate::sqlite::configrepo::ConfigRepoError;

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub(crate) async fn options(
    Extension(context): Extension<Arc<ServerContext>>,
) -> impl IntoResponse {
    let response = json!({
        "authentication": {
            "required": context.config.authentication_required,
            "types": [context.config.authentication_type.to_string()],
        }
    });
    Json(response)
}

pub(crate) async fn post(
    context: Extension<Arc<ServerContext>>,
    _session: Option<SessionExtractor>,
    form: axum::extract::Form<LoginForm>,
) -> impl IntoResponse {
    // No authentication required.
    if context.config.authentication_type == AuthenticationType::Anonymous {
        return (StatusCode::OK, Json(serde_json::json!({}))).into_response();
    }

    // We just take the username.
    if context.config.authentication_type == AuthenticationType::Username {
        let username = match &form.username {
            None => {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({}))).into_response();
            }
            Some(username) => username.to_owned(),
        };
        info!("Creating anonymous session for username={}", &username);
        let session = Arc::new(Session::anonymous(Some(username)));
        let _ = context.session_store.put(session.clone());
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": session.session_id,
            })),
        )
            .into_response();
    }

    if context.config.authentication_type == AuthenticationType::UsernamePassword {
        let username = match &form.username {
            None => {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({}))).into_response();
            }
            Some(username) => username.to_owned(),
        };
        let password = match &form.password {
            None => {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({}))).into_response();
            }
            Some(password) => password.to_owned(),
        };

        let user = match context
            .config_repo
            .get_user_by_username_password(&username, &password)
            .await
        {
            Ok(user) => user,
            Err(err) => match err {
                ConfigRepoError::UsernameNotFound(_)
                | ConfigRepoError::BadPassword(_)
                | ConfigRepoError::NoUser(_) => {
                    warn!("Login failure for username={}, error={:?}", &username, err);
                    return (StatusCode::UNAUTHORIZED, "").into_response();
                }
                _ => {
                    error!("Login failure for username={}, error={:?}", &username, err);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();
                }
            },
        };

        info!("Creating session for user {:?}", &username);
        let mut session = Session::new();
        session.username = Some(user.username);
        let session = Arc::new(session);
        context.session_store.put(session.clone()).unwrap();
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": session.session_id,
            })),
        )
            .into_response();
    }

    return (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "login failed"})),
    )
        .into_response();
}

pub(crate) async fn logout_new(
    context: Extension<Arc<ServerContext>>,
    SessionExtractor(session): SessionExtractor,
) -> impl IntoResponse {
    if let Some(session_id) = &session.session_id {
        if !context.session_store.delete(session_id) {
            warn!("Logout request for unknown session ID");
        } else {
            info!("User logged out: {:}", session.username());
        }
    }
    StatusCode::OK
}
