// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::agent::importer::EveBoxEventSink;
use crate::elastic::ElasticEventSink;
use crate::sqlite::importer::SqliteEventSink;

/// The importer interface, an enum wrapper around various implementations of an importer for Eve events.
#[derive(Clone)]
pub(crate) enum EventSink {
    EveBox(EveBoxEventSink),
    Elastic(ElasticEventSink),
    SQLite(SqliteEventSink),
}

impl EventSink {
    pub async fn submit(&mut self, event: serde_json::Value) -> anyhow::Result<bool> {
        match self {
            EventSink::EveBox(importer) => Ok(importer.submit(event).await?),
            EventSink::Elastic(importer) => Ok(importer.submit(event).await?),
            EventSink::SQLite(importer) => Ok(importer.submit(event).await?),
        }
    }

    pub async fn commit(&mut self) -> anyhow::Result<usize> {
        match self {
            EventSink::EveBox(importer) => importer.commit().await,
            EventSink::Elastic(importer) => importer.commit().await,
            EventSink::SQLite(importer) => importer.commit().await,
        }
    }

    pub fn pending(&self) -> usize {
        match self {
            EventSink::EveBox(importer) => importer.pending(),
            EventSink::Elastic(importer) => importer.pending(),
            EventSink::SQLite(importer) => importer.pending(),
        }
    }
}
