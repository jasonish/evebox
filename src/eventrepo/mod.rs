// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::datetime::DateTime;
use crate::importer::EventSink;
use crate::server::api::{self};
use crate::server::session::Session;
use crate::sqlite::eventrepo::SqliteEventRepo;
use crate::{elastic, queryparser};
use std::sync::Arc;
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum DatastoreError {
    #[error("unimplemented")]
    Unimplemented,
    #[error("event not found")]
    EventNotFound,
    #[error("elasticsearch: {0}")]
    ElasticSearchError(String),
    #[error("elasticsearch: {0}")]
    ElasticError(#[from] elastic::ElasticError),
    #[error("serde: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("time parser error: {0}")]
    DateTimeParse(#[from] crate::datetime::ParseError),

    #[error("sqlx: {0}")]
    SqlxError(#[from] sqlx::Error),

    // Fallback...
    #[error("error: {0}")]
    AnyhowError(#[from] anyhow::Error),
}

#[derive(Clone, Debug)]
pub(crate) struct StatsAggQueryParams {
    pub field: String,
    pub sensor_name: Option<String>,
    pub start_time: DateTime,
}

#[allow(unreachable_patterns)]
impl EventRepo {
    pub fn get_importer(&self) -> Option<EventSink> {
        match self {
            EventRepo::Elastic(ds) => ds.get_importer().map(EventSink::Elastic),
            EventRepo::SQLite(ds) => Some(EventSink::SQLite(ds.get_importer())),
        }
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.archive_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.archive_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.escalate_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.escalate_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.deescalate_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.get_event_by_id(event_id).await,
            EventRepo::SQLite(ds) => ds.get_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn alerts(
        &self,
        options: elastic::AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.alerts(options).await,
            EventRepo::SQLite(ds) => ds.alerts(options).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn archive_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.archive_by_alert_group(alert_group).await,
            EventRepo::SQLite(ds) => ds.archive_by_alert_group(alert_group).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.escalate_by_alert_group(alert_group, session).await,
            EventRepo::SQLite(ds) => ds.escalate_by_alert_group(alert_group).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.deescalate_by_alert_group(alert_group).await,
            EventRepo::SQLite(ds) => ds.deescalate_by_alert_group(alert_group).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn comment_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        comment: String,
        username: &str,
    ) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => {
                ds.comment_by_alert_group(alert_group, comment, username)
                    .await
            }
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn events(
        &self,
        params: EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.events(params).await,
            EventRepo::SQLite(ds) => ds.events(params).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        username: &str,
    ) -> Result<(), DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.comment_event_by_id(event_id, comment, username).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn group_by(
        &self,
        field: &str,
        size: usize,
        order: &str,
        query: Vec<queryparser::QueryElement>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        match self {
            EventRepo::Elastic(ds) => ds.group_by(field, size, order, query).await,
            EventRepo::SQLite(ds) => ds.group_by(field, size, order, query).await,
        }
    }
}
