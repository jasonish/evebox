// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::server::main::SessionExtractor;
use crate::server::session::Session;
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
            "type": "usernamepassword",
        }
    });
    Json(response)
}

pub(crate) async fn post(
    context: Extension<Arc<ServerContext>>,
    _session: Option<SessionExtractor>,
    form: axum::extract::Form<LoginForm>,
) -> impl IntoResponse {
    if !context.config.authentication_required {
        (StatusCode::OK, Json(serde_json::json!({}))).into_response()
    } else {
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
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": session.session_id,
            })),
        )
            .into_response()
    }
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
