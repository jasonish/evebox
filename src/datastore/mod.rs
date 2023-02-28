// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::importer::Importer;
use crate::server::api::{self};
use crate::server::session::Session;
use crate::sqlite::eventstore::SQLiteEventStore;
use crate::{elastic, querystring};
use std::sync::Arc;
use thiserror::Error;

mod stats;

#[derive(Default, Debug)]
pub struct EventQueryParams {
    pub query_string: Option<String>,
    pub order: Option<String>,
    pub min_timestamp: Option<time::OffsetDateTime>,
    pub max_timestamp: Option<time::OffsetDateTime>,
    pub event_type: Option<String>,
    pub size: Option<u64>,
    pub sort_by: Option<String>,
    pub query_string_elements: Vec<querystring::Element>,
}

pub enum Datastore {
    Elastic(crate::elastic::EventStore),
    SQLite(SQLiteEventStore),
}

#[derive(Error, Debug)]
pub enum DatastoreError {
    #[error("unimplemented")]
    Unimplemented,
    #[error("event not found")]
    EventNotFound,
    #[error("sqlite error: {0}")]
    SQLiteError(#[from] rusqlite::Error),
    #[error("generic datastore error")]
    GenericError(Box<dyn std::error::Error + Sync + Send>),
    #[error("elasticsearch: {0}")]
    ElasticSearchError(String),
    #[error("elasticsearch: {0}")]
    ElasticError(#[from] elastic::ElasticError),
    #[error("failed to parse histogram interval: {0}")]
    HistogramIntervalParseError(String),
    #[error("serde: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("database pool error: {0}")]
    DeadpoolError(#[from] deadpool_sqlite::PoolError),
    #[error("database pool interation error: {0}")]
    DeadpoolInteractionError(#[from] deadpool_sqlite::InteractError),
    #[error("time: {0}")]
    TimeParse(#[from] time::error::Parse),
    #[error("time: {0}")]
    TimeComponentRange(#[from] time::error::ComponentRange),
    #[error("sql: {0}")]
    FromSql(#[from] rusqlite::types::FromSqlError),

    // Fallback...
    #[error("error: {0}")]
    AnyhowError(#[from] anyhow::Error),
}

#[derive(Clone, Debug)]
pub struct StatsAggQueryParams {
    pub field: String,
    pub duration: time::Duration,
    pub sensor_name: Option<String>,
    pub start_time: time::OffsetDateTime,
}

#[allow(unreachable_patterns)]
impl Datastore {
    pub fn get_importer(&self) -> Option<Importer> {
        match self {
            Datastore::Elastic(ds) => Some(Importer::Elastic(ds.get_importer())),
            Datastore::SQLite(ds) => Some(Importer::SQLite(ds.get_importer())),
            _ => None,
        }
    }

    pub async fn archive_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.archive_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.archive_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.escalate_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.escalate_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: &str) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.deescalate_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.deescalate_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn get_event_by_id(
        &self,
        event_id: String,
    ) -> Result<Option<serde_json::Value>, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.get_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.get_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn alerts(
        &self,
        options: elastic::AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.alerts(options).await,
            Datastore::SQLite(ds) => ds.alerts(options).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn archive_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.archive_by_alert_group(alert_group).await,
            Datastore::SQLite(ds) => ds.archive_by_alert_group(alert_group).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn escalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
        session: Arc<Session>,
    ) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.escalate_by_alert_group(alert_group, session).await,
            Datastore::SQLite(ds) => ds.escalate_by_alert_group(alert_group).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn deescalate_by_alert_group(
        &self,
        alert_group: api::AlertGroupSpec,
    ) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.deescalate_by_alert_group(alert_group).await,
            Datastore::SQLite(ds) => ds.deescalate_by_alert_group(alert_group).await,
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
            Datastore::Elastic(ds) => {
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
            Datastore::Elastic(ds) => ds.events(params).await,
            Datastore::SQLite(ds) => ds.events(params).await,
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
            Datastore::Elastic(ds) => ds.comment_event_by_id(event_id, comment, username).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn group_by(
        &self,
        field: &str,
        size: usize,
        order: &str,
        q: Vec<querystring::Element>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.group_by(field, size, order, q).await,
            Datastore::SQLite(ds) => ds.group_by(field, size, order, q).await,
        }
    }
}

#[derive(Default, Debug)]
pub struct HistogramParameters {
    pub min_timestamp: Option<time::OffsetDateTime>,
    pub max_timestamp: Option<time::OffsetDateTime>,
    pub event_type: Option<String>,
    pub dns_type: Option<String>,
    pub address_filter: Option<String>,
    pub query_string: Option<String>,
    pub sensor_name: Option<String>,
    pub interval: Option<String>,
}

#[derive(Default, Debug)]
pub struct AggParameters {
    pub event_type: Option<String>,
    pub dns_type: Option<String>,
    pub query_string: Option<String>,
    pub address_filter: Option<String>,
    pub min_timestamp: Option<time::OffsetDateTime>,
    pub agg: String,
    pub size: u64,
}
