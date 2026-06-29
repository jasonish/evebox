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
    opensearch: bool,
}

impl ElasticEventSink {
    pub fn new(
        client: crate::elastic::Client,
        index: &str,
        no_index_suffix: bool,
        opensearch: bool,
    ) -> Self {
        Self {
            index: index.to_string(),
            queue: Vec::new(),
            client,
            no_index_suffix,
            opensearch,
        }
    }

    pub fn pending(&self) -> usize {
        self.queue.len() / 2
    }

    pub async fn submit(&mut self, mut event: serde_json::Value) -> anyhow::Result<bool> {
        let ts = event.datetime().unwrap();
        let st: std::time::SystemTime = ts.to_systemtime();

        let index = select_index(
            &self.index,
            self.no_index_suffix,
            self.opensearch,
            event["event_type"].as_str(),
            &ts.yyyymmdd("."),
        );
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

/// Select the destination index for an event.
///
/// On OpenSearch, stats events are routed to their own `{base}-stats-{date}`
/// index so the large number of `stats.*` counter fields stay out of the main
/// event index mapping. Both `{base}-*` (search) and `{base}*` (retention) still
/// match the stats index, so queries and index retention pick it up without any
/// further change.
///
/// The split is OpenSearch-only; on Elasticsearch everything stays in the single
/// daily index. It is also skipped in date-less mode (`no_index_suffix`), where
/// the search pattern is the exact index name and a separate stats index would
/// be unsearchable.
fn select_index(
    base: &str,
    no_index_suffix: bool,
    opensearch: bool,
    event_type: Option<&str>,
    date: &str,
) -> String {
    if no_index_suffix {
        base.to_string()
    } else if opensearch && event_type == Some("stats") {
        format!("{base}-stats-{date}")
    } else {
        format!("{base}-{date}")
    }
}

#[cfg(test)]
mod tests {
    use super::{ElasticEventSink, select_index};

    #[test]
    fn opensearch_routes_stats_to_their_own_index() {
        assert_eq!(
            select_index("logstash", false, true, Some("stats"), "2026.06.28"),
            "logstash-stats-2026.06.28"
        );
    }

    #[test]
    fn elasticsearch_keeps_stats_in_the_main_index() {
        // The split is OpenSearch-only.
        assert_eq!(
            select_index("logstash", false, false, Some("stats"), "2026.06.28"),
            "logstash-2026.06.28"
        );
    }

    #[test]
    fn non_stats_events_go_to_the_main_index() {
        assert_eq!(
            select_index("logstash", false, true, Some("alert"), "2026.06.28"),
            "logstash-2026.06.28"
        );
        assert_eq!(
            select_index("logstash", false, true, None, "2026.06.28"),
            "logstash-2026.06.28"
        );
    }

    #[test]
    fn date_less_mode_keeps_a_single_index() {
        assert_eq!(
            select_index("logstash", true, true, Some("stats"), "2026.06.28"),
            "logstash"
        );
        assert_eq!(
            select_index("logstash", true, true, Some("alert"), "2026.06.28"),
            "logstash"
        );
    }

    // Guard the wiring between submit() and select_index(): a future change to
    // the field submit() reads (e.g. the wrong key) would otherwise route stats
    // back into the main index without any test noticing. submit() does no
    // network IO, so this needs no live datastore.
    #[tokio::test]
    async fn submit_writes_the_event_type_specific_index_into_the_bulk_header() {
        let client = crate::elastic::ClientBuilder::new("http://localhost:9200").build();
        let mut sink = ElasticEventSink::new(client, "logstash", false, true);

        sink.submit(serde_json::json!({
            "timestamp": "2026-06-28T12:00:00.000000+0000",
            "event_type": "stats",
        }))
        .await
        .unwrap();
        sink.submit(serde_json::json!({
            "timestamp": "2026-06-28T12:00:00.000000+0000",
            "event_type": "alert",
        }))
        .await
        .unwrap();

        // queue is [stats header, stats event, alert header, alert event].
        let stats_header: serde_json::Value = serde_json::from_str(&sink.queue[0]).unwrap();
        assert_eq!(
            stats_header["create"]["_index"],
            "logstash-stats-2026.06.28"
        );
        let alert_header: serde_json::Value = serde_json::from_str(&sink.queue[2]).unwrap();
        assert_eq!(alert_header["create"]["_index"], "logstash-2026.06.28");
    }
}
