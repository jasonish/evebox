// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::client::BulkResponse;
use crate::eve::Eve;
use tracing::{error, trace};

#[derive(Clone, Debug)]
pub(crate) struct ElasticEventSink {
    index: String,
    queue: Vec<String>,
    client: crate::elastic::Client,
    no_index_suffix: bool,
}

impl ElasticEventSink {
    pub fn new(client: crate::elastic::Client, index: &str, no_index_suffix: bool) -> Self {
        Self {
            index: index.to_string(),
            queue: Vec::new(),
            client,
            no_index_suffix,
        }
    }

    pub fn pending(&self) -> usize {
        self.queue.len() / 2
    }

    pub async fn submit(&mut self, mut event: serde_json::Value) -> anyhow::Result<bool> {
        let ts = event.datetime().unwrap();
        let st: std::time::SystemTime = ts.to_systemtime();

        let index = if self.no_index_suffix {
            self.index.clone()
        } else {
            format!("{}-{}", self.index, ts.yyyymmdd("."))
        };
        let event_id = ulid::Ulid::from_datetime(st).to_string();
        let at_timestamp = ts.to_elastic();
        event["@timestamp"] = at_timestamp.into();

        let header = serde_json::json!({
            "create": {
                "_index": index,
                "_id": event_id,
            }
        });

        self.queue.push(header.to_string());
        self.queue.push(event.to_string());

        Ok(false)
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        let n = self.pending();
        self.queue.push("".to_string());
        let mut body = self.queue.join("\n");
        body.push('\n');
        trace!(
            "Sending Elasticsearch bulk request: bytes={}, events={}",
            body.len(),
            self.queue.len() / 2,
        );
        let request = self.client.post("_bulk")?.body(body);
        let response = request.send().await?;
        let body_text = response.text().await?;
        let body: BulkResponse = serde_json::from_str(&body_text)?;
        if body.has_error() {
            if let Some(error) = body.first_error() {
                error!(
                    "Elasticsearch one of more errors to the commit operation, first error: {}",
                    error
                );
                return Err(anyhow!("elasticsearch commit error: {}", error));
            } else {
                error!("Elasticsearch reported errors during commit: {}", body_text);
                return Err(anyhow!("elasticsearch commit error: {}", body_text));
            }
        }
        self.queue.truncate(0);
        Ok(n)
    }
}
