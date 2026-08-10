// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Elasticsearch auto-archiver.
//!
//! For Elasticsearch, particularly where events are added by an
//! external process, matching filters are queued here when alerts
//! are retrieved so the matching historical events can be archived.

use crate::prelude::*;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::sqlite::configdb::EventFilter;

use super::ElasticEventRepo;

pub(crate) struct AutoArchiveProcessor {
    repo: ElasticEventRepo,
    rx: UnboundedReceiver<EventFilter>,
}

impl AutoArchiveProcessor {
    pub fn start(repo: ElasticEventRepo) -> UnboundedSender<EventFilter> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<EventFilter>();
        tokio::spawn(async move {
            Self { repo, rx }.run().await;
        });
        tx
    }

    async fn run(mut self) {
        while let Some(filter) = self.rx.recv().await {
            trace!("Auto-archiving by filter: {:?}", &filter);
            if let Err(err) = self.repo.auto_archive_by_filter(&filter).await {
                warn!("Failed to auto-archive alerts: {:?}", err);
            }
        }
    }
}
