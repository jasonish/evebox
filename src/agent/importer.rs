// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// EveBox agent import. For importing events to an EveBox server.

use crate::agent::client::Client;
use tracing::trace;

// The server, 0.17.0+ should have a receive size limit of 32 megabytes. We'll do climate side
// limiting at 16 MB.
const LIMIT: usize = 1024 * 1024 * 16;

#[derive(Debug, Clone)]
pub(crate) struct EveBoxEventSink {
    pub client: Client,
    pub queue: Vec<String>,
    pub size: usize,
}

impl EveBoxEventSink {
    pub fn new(client: Client) -> Self {
        Self {
            queue: Vec::new(),
            client,
            size: 0,
        }
    }

    /// The result will be true if the user should `commit` before submitting new events.
    pub async fn submit(
        &mut self,
        event: serde_json::Value,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let as_string = event.to_string();
        self.size += as_string.len();
        self.queue.push(as_string);
        Ok(self.size > LIMIT)
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub fn truncate(&mut self) {
        self.queue.truncate(0);
        self.size = 0;
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        let n = self.queue.len();
        let body = self.queue.join("\n");
        let size = body.len();
        trace!("Committing {} events (bytes: {})", n, size);
        let r = self.client.post("api/1/submit")?.body(body).send().await?;
        let status_code = r.status();
        if status_code != 200 {
            let response_body = r.text().await?;
            if !response_body.is_empty() {
                if let Ok(error) = serde_json::from_str::<serde_json::Value>(&response_body) {
                    if let serde_json::Value::String(error) = &error["error"] {
                        return Err(anyhow!("{}", error));
                    }
                }
                return Err(anyhow!("{}", response_body));
            }
            return Err(anyhow!("Server returned status code {}", status_code));
        }
        self.truncate();
        Ok(n)
    }
}
