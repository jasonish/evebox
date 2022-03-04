// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish
// EveBox agent import. For importing events to an EveBox server.

use crate::agent::client::Client;
use crate::eve::eve::EveJson;
use tracing::trace;

#[derive(Debug, Clone)]
pub struct EveboxImporter {
    pub client: Client,
    pub queue: Vec<String>,
}

impl EveboxImporter {
    pub fn new(client: Client) -> Self {
        Self {
            queue: Vec::new(),
            client: client,
        }
    }

    pub async fn submit(
        &mut self,
        event: EveJson,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.queue.push(event.to_string());
        Ok(())
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
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
        self.queue.truncate(0);
        Ok(n)
    }
}
