// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

use crate::elastic;
use crate::importer::Importer;
use crate::server::api;
use crate::server::session::Session;
use crate::sqlite::eventstore::SQLiteEventStore;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use thiserror::Error;

#[derive(Default, Debug)]
pub struct EventQueryParams {
    pub query_string: Option<String>,
    pub order: Option<String>,
    pub min_timestamp: Option<time::OffsetDateTime>,
    pub max_timestamp: Option<time::OffsetDateTime>,
    pub event_type: Option<String>,
    pub size: Option<u64>,
    pub sort_by: Option<String>,
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
    SQLiteError(rusqlite::Error),
    #[error("generic datastore error")]
    GenericError(Box<dyn std::error::Error + Sync + Send>),
    #[error("elasticsearch: {0}")]
    ElasticSearchError(String),
    #[error("elasticsearch: {0}")]
    ElasticError(elastic::ElasticError),
    #[error("time parse error")]
    TimeParseError(time::error::Parse),
    #[error("failed to parse event")]
    EventParseError,
    #[error("failed to parse histogram interval: {0}")]
    HistogramIntervalParseError(String),
    #[error("error: {0}")]
    AnyhowError(anyhow::Error),
}

impl From<Box<dyn std::error::Error + Sync + Send>> for DatastoreError {
    fn from(err: Box<dyn std::error::Error + Sync + Send>) -> Self {
        DatastoreError::GenericError(err)
    }
}

impl From<time::error::Parse> for DatastoreError {
    fn from(err: time::error::Parse) -> Self {
        DatastoreError::TimeParseError(err)
    }
}

impl From<anyhow::Error> for DatastoreError {
    fn from(err: anyhow::Error) -> Self {
        DatastoreError::AnyhowError(err)
    }
}

#[derive(Clone, Debug)]
pub struct StatsAggQueryParams {
    pub field: String,
    pub duration: time::Duration,
    pub interval: time::Duration,
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

    pub async fn alert_query(
        &self,
        options: elastic::AlertQueryOptions,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.alert_query(options).await,
            Datastore::SQLite(ds) => ds.alert_query(options).await,
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

    pub async fn event_query(
        &self,
        params: EventQueryParams,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.event_query(params).await,
            Datastore::SQLite(ds) => ds.event_query(params).await,
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

    pub async fn histogram(
        &self,
        params: HistogramParameters,
    ) -> Result<serde_json::Value, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.histogram(params).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn agg(&self, params: AggParameters) -> Result<JsonValue, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.agg(params).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn flow_histogram(
        &self,
        params: FlowHistogramParameters,
    ) -> Result<JsonValue, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.flow_histogram(params).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn report_dhcp(
        &self,
        what: &str,
        params: &EventQueryParams,
    ) -> Result<JsonValue, DatastoreError> {
        match self {
            Datastore::Elastic(ds) => elastic::report::dhcp::dhcp_report(ds, what, params).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }
}

#[derive(Default, Debug)]
pub struct HistogramParameters {
    pub min_timestamp: Option<time::OffsetDateTime>,
    pub max_timestamp: Option<time::OffsetDateTime>,
    pub interval: Option<HistogramInterval>,
    pub event_type: Option<String>,
    pub dns_type: Option<String>,
    pub address_filter: Option<String>,
    pub query_string: Option<String>,
    pub sensor_name: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HistogramInterval {
    Minute,
    Hour,
    Day,
}

impl HistogramInterval {
    pub fn from_str(s: &str) -> Result<HistogramInterval, DatastoreError> {
        match s {
            "minute" => Ok(HistogramInterval::Minute),
            "hour" => Ok(HistogramInterval::Hour),
            "day" => Ok(HistogramInterval::Day),
            _ => Err(DatastoreError::HistogramIntervalParseError(s.to_string())),
        }
    }
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

pub struct FlowHistogramParameters {
    pub mints: Option<time::OffsetDateTime>,
    pub interval: Option<String>,
    pub query_string: Option<String>,
}

#[cfg(test)]
mod test {
    use super::HistogramInterval;

    #[test]
    fn test_histogram_interval_from_str() {
        let r = HistogramInterval::from_str("minute");
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), HistogramInterval::Minute);
        assert!(HistogramInterval::from_str("bad").is_err());
    }
}
