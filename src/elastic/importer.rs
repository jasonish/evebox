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

use super::client::BulkResponse;
use crate::eve::filters::AutoArchiveFilter;
use crate::eve::Eve;
use crate::prelude::*;
use time::macros::format_description;

#[derive(Clone, Debug)]
pub struct Importer {
    index: String,
    queue: Vec<String>,
    client: crate::elastic::Client,
    no_index_suffix: bool,
    auto_archive_filter: AutoArchiveFilter,
}

impl Importer {
    pub fn new(client: crate::elastic::Client, index: &str, no_index_suffix: bool) -> Self {
        Self {
            index: index.to_string(),
            queue: Vec::new(),
            client: client,
            no_index_suffix,
            auto_archive_filter: AutoArchiveFilter::default(),
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
        let st: std::time::SystemTime = ts.into();

        let index = if self.no_index_suffix {
            self.index.clone()
        } else {
            let formatter = format_description!("[year].[month].[day]");
            format!(
                "{}-{}",
                self.index,
                ts.to_offset(time::UtcOffset::UTC)
                    .format(&formatter)
                    .unwrap()
            )
        };
        let event_id = ulid::Ulid::from_datetime(st).to_string();
        let at_timestamp = crate::elastic::format_timestamp(ts);
        event["@timestamp"] = at_timestamp.into();
        self.auto_archive_filter.run(&mut event);

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
