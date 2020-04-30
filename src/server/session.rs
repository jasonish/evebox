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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use rand::RngCore;

use crate::sqlite::configrepo::User;

pub struct SessionStore {
    cache: Mutex<HashMap<String, Arc<Session>>>,
}

impl SessionStore {
    pub fn new() -> SessionStore {
        SessionStore {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn put(&self, session: Arc<Session>) -> Result<()> {
        let mut cache = self.cache.lock().unwrap();
        if cache.insert(session.session_id.clone(), session).is_some() {
            return Err(anyhow!("duplicate session-id"));
        }
        Ok(())
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<Session>> {
        let cache = self.cache.lock().unwrap();
        if let Some(session) = cache.get(session_id) {
            Some(session.clone())
        } else {
            None
        }
    }

    pub fn delete(&self, session_id: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(session_id).is_some()
    }
}

#[derive(Debug)]
pub struct Session {
    pub session_id: String,
    pub user: Option<User>,
    pub username: Option<String>,
    pub inner: Mutex<SessionInner>,
}

#[derive(Debug)]
pub struct SessionInner {
    pub hits: u64,
}

impl Session {
    pub fn new() -> Session {
        let session_id = generate_session_id();
        Session {
            session_id,
            username: None,
            inner: Mutex::new(SessionInner { hits: 0 }),
            user: None,
        }
    }

    pub fn is_anonymous(&self) -> bool {
        self.user.is_none()
    }

    pub fn username(&self) -> &str {
        if let Some(username) = &self.username {
            return &username;
        } else {
            return "<anonymous>";
        }
    }
}

pub fn generate_session_id() -> String {
    let mut rng = rand::thread_rng();
    let mut buf = vec![0; 256];
    rng.fill_bytes(&mut buf);
    base64::encode(&buf)
}
