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

use super::client::BulkResponse;
use crate::eve::Eve;
use crate::logger::log;

#[derive(Clone, Debug)]
pub struct Importer {
    index: String,
    queue: Vec<String>,
    client: crate::elastic::Client,
    no_index_suffix: bool,
}

impl Importer {
    pub fn new(client: crate::elastic::Client, index: &str, no_index_suffix: bool) -> Self {
        Self {
            index: index.to_string(),
            queue: Vec::new(),
            client: client,
            no_index_suffix,
        }
    }

    pub fn pending(&self) -> usize {
        self.queue.len() / 2
    }

    pub async fn submit(
        &mut self,
        mut event: serde_json::Value,
    ) -> anyhow::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ts = event.timestamp().unwrap();
        let index = if self.no_index_suffix {
            self.index.clone()
        } else {
            format!("{}-{}", self.index, ts.format("%Y.%m.%d"))
        };
        let event_id = ulid::Ulid::from_datetime(ts).to_string();
        let at_timestamp = crate::elastic::format_timestamp(ts);
        event["@timestamp"] = at_timestamp.into();

        let mut header = serde_json::json!({
            "create": {
                "_index": index,
                "_id": event_id,
            }
        });

        let version = self.client.get_version().await?;
        if version.major < 7 {
            header["create"]["_type"] = "_doc".into();
        }

        self.queue.push(header.to_string());
        self.queue.push(event.to_string());

        Ok(())
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        let n = self.pending();
        self.queue.push("".to_string());
        let mut body = self.queue.join("\n");
        body.push('\n');
        log::trace!(
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
                log::error!(
                    "Elasticsearch one of more errors to the commit operation, first error: {}",
                    error
                );
                return Err(anyhow!("elasticsearch commit error: {}", error));
            } else {
                log::error!("Elasticsearch reported errors during commit: {}", body_text);
                return Err(anyhow!("elasticsearch commit error: {}", body_text));
            }
        }
        self.queue.truncate(0);
        Ok(n)
    }
}
