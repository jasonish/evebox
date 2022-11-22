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

use anyhow::Result;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct SessionStore {
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

#[derive(Debug, Default)]
pub struct Session {
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

    pub fn anonymous(username: Option<String>) -> Session {
        let session_id = generate_session_id();
        Session {
            username,
            session_id: Some(session_id),
        }
    }

    pub fn username(&self) -> &str {
        if let Some(username) = &self.username {
            return username;
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
