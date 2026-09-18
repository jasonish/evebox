// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::importer::EventSink;
use crate::prelude::*;
use crate::queryparser;
use crate::queryparser::QueryElement;
use crate::server::autoarchive::AutoArchive;
use crate::server::session::Session;
use crate::sqlite::eventrepo::SqliteEventRepo;
use serde::Serialize;
use std::sync::Arc;

mod stats;

#[derive(Default, Debug)]
pub(crate) struct EventQueryParams {
    pub order: Option<String>,
    pub from: Option<DateTime>,
    pub to: Option<DateTime>,
    pub event_type: Option<String>,
    pub sensor: Option<String>,
    pub size: Option<u64>,
    pub sort_by: Option<String>,
    pub query_string: Vec<queryparser::QueryElement>,
}

pub(crate) enum EventRepo {
    Elastic(crate::elastic::ElasticEventRepo),
    SQLite(SqliteEventRepo),
}

/// Options for an alert inbox query.
#[derive(Default, Debug, Clone)]
pub(crate) struct AlertQueryOptions {
    pub timestamp_gte: Option<DateTime>,
    pub query_string: Option<String>,
    pub tags: Vec<String>,
    pub sensor: Option<String>,
    pub timeout: Option<u64>,
}

/// Identifies a group of alerts (as aggregated in the inbox) for
/// bulk archive and escalation operations.
#[derive(Deserialize, Debug, Clone)]
pub(crate) struct AlertGroupSpec {
    pub signature_id: u64,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub sensor: Option<String>,
    pub dns_rrname: Option<String>,
    pub tls_sni: Option<String>,
    pub min_timestamp: String,
    pub max_timestamp: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StatsAggQueryParams {
    pub field: String,
    pub sensor_name: Option<String>,
    pub start_time: DateTime,
    pub end_time: DateTime,
}

#[derive(Default, Debug, Serialize)]
pub(crate) struct AlertsResult {
    pub(crate) ecs: bool,
    pub(crate) events: Vec<AggAlert>,
    pub(crate) took: u64,
    pub(crate) timed_out: bool,
    pub(crate) min_timestamp: Option<crate::datetime::DateTime>,
    pub(crate) max_timestamp: Option<crate::datetime::DateTime>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AggAlert {
    #[serde(rename = "_id")]
    pub(crate) id: String,
    #[serde(rename = "_source")]
    pub(crate) source: serde_json::Value,
    #[serde(rename = "_metadata")]
    pub(crate) metadata: AggAlertMetadata,
}

// Could be merged into AggAlert, but requires client side changes.
#[derive(Debug, Serialize)]
pub(crate) struct AggAlertMetadata {
    pub(crate) count: u64,
    pub(crate) escalated_count: u64,
    pub(crate) min_timestamp: DateTime,
    pub(crate) max_timestamp: DateTime,
}

impl EventRepo {
    pub(crate) fn get_importer(&self) -> Option<EventSink> {
        match self {
            EventRepo::Elastic(ds) => ds.get_importer().map(EventSink::Elastic),
            EventRepo::SQLite(ds) => Some(EventSink::SQLite(ds.get_importer())),
        }
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => {
                ds.archive_event_by_id(event_id).await?;
                Ok(())
            }
            EventRepo::SQLite(ds) => ds.archive_event_by_id(event_id).await,
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => {
                ds.escalate_event_by_id(event_id).await?;
                Ok(())
            }
            EventRepo::SQLite(ds) => ds.escalate_event_by_id(event_id).await,
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.deescalate_event_by_id(event_id).await,
        }
    }

    pub async fn get_event_by_id(&self, event_id: String) -> Result<Option<serde_json::Value>> {
        match self {
            EventRepo::Elastic(ds) => ds.get_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.get_event_by_id(event_id).await,
        }
    }

    pub async fn alerts(
        &self,
        options: AlertQueryOptions,
        auto_archive: Arc<RwLock<AutoArchive>>,
    ) -> Result<AlertsResult> {
        match self {
            EventRepo::Elastic(ds) => ds.alerts(options, auto_archive).await,
            EventRepo::SQLite(ds) => ds.alerts(options).await,
        }
    }

    pub async fn archive_by_alert_group(&self, alert_group: AlertGroupSpec) -> Result<u64> {
        match self {
            EventRepo::Elastic(ds) => ds.archive_by_alert_group(alert_group).await,
            EventRepo::SQLite(ds) => ds.archive_by_alert_group(alert_group).await,
        }
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => {
                ds.escalate_by_alert_group(alert_group, session).await?;
                Ok(())
            }
            EventRepo::SQLite(ds) => ds.escalate_by_alert_group(alert_group, session).await,
        }
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_by_alert_group(alert_group, session).await,
            EventRepo::SQLite(ds) => ds.deescalate_by_alert_group(alert_group, session).await,
        }
    }

    pub async fn events(&self, params: EventQueryParams) -> Result<serde_json::Value> {
        match self {
            EventRepo::Elastic(ds) => ds.events(params).await,
            EventRepo::SQLite(ds) => ds.events(params).await,
        }
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        session: Arc<Session>,
    ) -> Result<()> {
        match self {
            EventRepo::Elastic(ds) => {
                ds.comment_event_by_id(event_id, comment, session).await?;
                Ok(())
            }
            EventRepo::SQLite(ds) => ds.comment_event_by_id(event_id, comment, session).await,
        }
    }

    pub async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<serde_json::Value>> {
        match self {
            EventRepo::Elastic(ds) => Ok(ds.agg(field, size, order, query).await?),
            EventRepo::SQLite(ds) => Ok(ds.agg(field, size, order, query).await?),
        }
    }

    pub(crate) async fn earliest_timestamp(&self) -> Result<Option<DateTime>> {
        match self {
            EventRepo::Elastic(repo) => repo.earliest_timestamp().await,
            EventRepo::SQLite(repo) => repo.earliest_timestamp().await,
        }
    }

    /// Count the events matching a query string.
    pub(crate) async fn count(&self, query: &[QueryElement]) -> Result<u64> {
        match self {
            EventRepo::Elastic(repo) => repo.count(query).await,
            EventRepo::SQLite(repo) => repo.count(query).await,
        }
    }

    pub(crate) async fn histogram_time(
        &self,
        interval: Option<u64>,
        query: &[QueryElement],
    ) -> Result<Vec<serde_json::Value>> {
        match self {
            EventRepo::Elastic(repo) => repo.histogram_time(interval, query).await,
            EventRepo::SQLite(repo) => repo.histogram_time(interval, query).await,
        }
    }

    /// The distinct event types matching a query string.
    ///
    /// Elasticsearch currently ignores the query and returns every
    /// event type in the index.
    pub(crate) async fn get_event_types(&self, query: &[QueryElement]) -> Result<Vec<String>> {
        match self {
            EventRepo::Elastic(repo) => repo.get_event_types().await,
            EventRepo::SQLite(repo) => repo.get_event_types(query).await,
        }
    }

    pub(crate) async fn get_sensors(&self) -> Result<Vec<String>> {
        match self {
            EventRepo::Elastic(repo) => repo.get_sensors().await,
            EventRepo::SQLite(repo) => repo.get_sensors().await,
        }
    }

    pub(crate) async fn dhcp_ack(
        &self,
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        match self {
            EventRepo::Elastic(repo) => repo.dhcp_ack(earliest, sensor).await,
            EventRepo::SQLite(repo) => repo.dhcp_ack(earliest, sensor).await,
        }
    }

    pub(crate) async fn dhcp_request(
        &self,
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        match self {
            EventRepo::Elastic(repo) => repo.dhcp_request(earliest, sensor).await,
            EventRepo::SQLite(repo) => repo.dhcp_request(earliest, sensor).await,
        }
    }

    pub(crate) async fn dns_reverse_lookup(
        &self,
        before: Option<DateTime>,
        sensor: Option<String>,
        src_ip: String,
        dest_ip: String,
    ) -> Result<serde_json::Value> {
        match self {
            EventRepo::Elastic(repo) => {
                repo.dns_reverse_lookup(before, sensor, src_ip, dest_ip)
                    .await
            }
            EventRepo::SQLite(repo) => {
                repo.dns_reverse_lookup(before, sensor, src_ip, dest_ip)
                    .await
            }
        }
    }
}
