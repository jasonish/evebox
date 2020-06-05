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
use std::sync::Arc;

use serde::Deserialize;

use crate::logger::log;
use crate::server::response::Response;
use crate::server::session::Session;
use crate::server::AuthenticationType;
use crate::server::ServerContext;

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub async fn options(context: Arc<ServerContext>) -> Result<impl warp::Reply, Infallible> {
    let response = json!({
        "authentication": {
            "required": context.config.authentication_required,
            "types": [context.config.authentication_type.to_string()],
        }
    });
    Ok(Response::Json(response))
}

pub async fn post(
    context: Arc<ServerContext>,
    form: LoginForm,
) -> Result<impl warp::Reply, Infallible> {
    let session = match (
        &context.config.authentication_type,
        &form.username,
        &form.password,
    ) {
        (AuthenticationType::Anonymous, _, _) => {
            let session = Session::new();
            Some(session)
        }
        (AuthenticationType::Username, Some(username), _) => {
            let mut session = Session::new();
            session.username = Some(username.to_string());
            Some(session)
        }
        (AuthenticationType::UsernamePassword, Some(username), Some(password)) => match context
            .config_repo
            .get_user_by_username_password(username, password)
            .await
        {
            Ok(user) => {
                let mut session = Session::new();
                session.username = Some(user.username.clone());
                session.user = Some(user);
                Some(session)
            }
            Err(err) => {
                log::warn!("Login failed for username {}: error={}", username, err);
                None
            }
        },
        _ => None,
    };

    if let Some(session) = session {
        let session = Arc::new(session);
        if let Err(err) = context.session_store.put(session.clone()) {
            log::error!("Failed to add new session to session store: {}", err);
            return Ok(Response::InternalError(err.to_string()));
        }
        let response = json!({
            "session_id": session.session_id,
        });
        return Ok(Response::Json(response));
    }

    return Ok(Response::Unauthorized);
}

pub async fn logout(
    context: Arc<ServerContext>,
    session: Arc<Session>,
) -> Result<impl warp::Reply, Infallible> {
    if !context.session_store.delete(&session.session_id) {
        log::warn!("Logout request for unknown session ID");
    } else {
        log::info!("User logged out: {:}", session.username());
    }
    Ok(Response::Ok)
}
