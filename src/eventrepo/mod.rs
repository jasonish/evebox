// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::error::AppError;
use crate::importer::EventSink;
use crate::server::api;
use crate::server::session::Session;
use crate::sqlite::eventrepo::SqliteEventRepo;
use crate::{elastic, queryparser};
use serde::Serialize;
use std::sync::Arc;

mod stats;

#[derive(Default, Debug)]
pub(crate) struct EventQueryParams {
    pub order: Option<String>,
    pub min_timestamp: Option<DateTime>,
    pub max_timestamp: Option<DateTime>,
    pub event_type: Option<String>,
    pub size: Option<u64>,
    pub sort_by: Option<String>,
    pub query_string: Vec<queryparser::QueryElement>,
}

pub enum EventRepo {
    Elastic(crate::elastic::ElasticEventRepo),
    SQLite(SqliteEventRepo),
}

#[derive(Clone, Debug)]
pub(crate) struct StatsAggQueryParams {
    pub field: String,
    pub sensor_name: Option<String>,
    pub start_time: DateTime,
}

#[derive(Debug, Serialize)]
pub(crate) struct AlertsResult {
    pub(crate) ecs: bool,
    pub(crate) events: Vec<AggAlert>,
    pub(crate) took: u64,
    pub(crate) timed_out: bool,
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

#[allow(unreachable_patterns)]
impl EventRepo {
    pub fn get_importer(&self) -> Option<EventSink> {
        match self {
            EventRepo::Elastic(ds) => ds.get_importer().map(EventSink::Elastic),
            EventRepo::SQLite(ds) => Some(EventSink::SQLite(ds.get_importer())),
        }
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.archive_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.archive_event_by_id(event_id).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.escalate_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.escalate_event_by_id(event_id).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.deescalate_event_by_id(event_id).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.get_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.get_event_by_id(event_id).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn alerts(
        &self,
        options: elastic::AlertQueryOptions,
    ) -> Result<AlertsResult, AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.alerts(options).await,
            EventRepo::SQLite(ds) => ds.alerts(options).await,
        }
    }

    pub async fn archive_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.archive_by_alert_group(alert_group).await,
            EventRepo::SQLite(ds) => ds.archive_by_alert_group(alert_group).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.escalate_by_alert_group(alert_group, session).await,
            EventRepo::SQLite(ds) => ds.escalate_by_alert_group(session, alert_group).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn deescalate_by_alert_group(
        &self,
        session: Arc<Session>,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_by_alert_group(alert_group).await,
            EventRepo::SQLite(ds) => ds.deescalate_by_alert_group(session, alert_group).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn events(&self, params: EventQueryParams) -> Result<serde_json::Value, AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.events(params).await,
            EventRepo::SQLite(ds) => ds.events(params).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        session: Arc<Session>,
    ) -> Result<(), AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.comment_event_by_id(event_id, comment, session).await,
            EventRepo::SQLite(ds) => ds.comment_event_by_id(event_id, comment, session).await,
            _ => Err(AppError::Unimplemented),
        }
    }

    pub async fn agg(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        match self {
            EventRepo::Elastic(ds) => ds.agg(field, size, order, query).await,
            EventRepo::SQLite(ds) => ds.agg(field, size, order, query).await,
        }
    }
}
