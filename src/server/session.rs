// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) struct SessionStore {
    cache: Mutex<HashMap<String, Arc<Session>>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn put(&self, session: Arc<Session>) -> Result<()> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(session_id) = &session.session_id {
            if cache.insert(session_id.to_string(), session).is_some() {
                return Err(anyhow!("duplicate session-id"));
            }
        }
        Ok(())
    }

    pub fn get(&self, session_id: &str) -> Option<Arc<Session>> {
        let cache = self.cache.lock().unwrap();
        cache.get(session_id).cloned()
    }

    pub fn delete(&self, session_id: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(session_id).is_some()
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Session {
    pub session_id: Option<String>,
    pub username: Option<String>,
}

impl Session {
    pub fn new() -> Session {
        let session_id = generate_session_id();
        Session {
            session_id: Some(session_id),
            username: None,
        }
    }

    pub fn with_username(username: &str) -> Self {
        let session_id = generate_session_id();
        Session {
            session_id: Some(session_id),
            username: Some(username.to_string()),
        }
    }

    pub fn anonymous(username: Option<String>) -> Session {
        let session_id = generate_session_id();
        Session {
            username,
            session_id: Some(session_id),
        }
    }

    // pub fn username(&self) -> &str {
    //     if let Some(username) = &self.username {
    //         username
    //     } else {
    //         "<anonymous>"
    //     }
    // }
}

pub(crate) fn generate_session_id() -> String {
    let mut rng = rand::thread_rng();
    let mut buf = vec![0; 256];
    rng.fill_bytes(&mut buf);
    base64::encode(&buf)
}
