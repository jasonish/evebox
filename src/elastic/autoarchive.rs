// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Elasticsearch auto-archiver.
//!
//! For Elasticsearch, particular where events are added by an
//! external process, this is a task that accepts `AlertGroupSpec`
//! structs on a channel and archives the matching events.
//!
//! The idea is that when retrieving alerts, when alerts match an
//! auto-archive filter, the handler will send the match here to be
//! queued and process.

use crate::prelude::*;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::server::api::AlertGroupSpec;

use super::ElasticEventRepo;

pub(crate) struct AutoArchiveProcessor {
    repo: ElasticEventRepo,
    rx: UnboundedReceiver<AlertGroupSpec>,
}

impl AutoArchiveProcessor {
    pub fn start(repo: ElasticEventRepo) -> UnboundedSender<AlertGroupSpec> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AlertGroupSpec>();
        tokio::spawn(async move {
            Self { repo, rx }.run().await;
        });
        tx
    }

    async fn run(mut self) {
        while let Some(x) = self.rx.recv().await {
            trace!("Auto-archiving {:?}", &x);
            if let Err(err) = self.repo.auto_archive_by_alert_group(x).await {
                warn!("Failed to auto-archive alerts: {:?}", err);
            }
        }
    }
}
