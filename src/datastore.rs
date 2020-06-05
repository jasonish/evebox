// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::elastic;
use crate::importer::Importer;
use crate::server::api;
use crate::server::session::Session;
use crate::sqlite::eventstore::SQLiteEventStore;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use thiserror::Error;

type DateTime = chrono::DateTime<chrono::Utc>;

#[derive(Default, Debug)]
pub struct EventQueryParams {
    pub query_string: Option<String>,
    pub order: Option<String>,
    pub min_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub max_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub event_type: Option<String>,
    pub size: Option<u64>,
    pub sort_by: Option<String>,
}

pub enum Datastore {
    None,
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
    #[error("elastic search error")]
    ElasticSearchError(String),
    #[error("elasticsearch: {0}")]
    ElasticError(elastic::ElasticError),
    #[error("failed to parse timestamp")]
    TimestampParseError(chrono::format::ParseError),
    #[error("failed to parse event")]
    EventParseError,
    #[error("failed to parse histogram interval: {0}")]
    HistogramIntervalParseError(String),
}

impl warp::reject::Reject for DatastoreError {}

impl From<DatastoreError> for warp::Rejection {
    fn from(err: DatastoreError) -> Self {
        warp::reject::custom(err)
    }
}

impl From<Box<dyn std::error::Error + Sync + Send>> for DatastoreError {
    fn from(err: Box<dyn std::error::Error + Sync + Send>) -> Self {
        DatastoreError::GenericError(err)
    }
}

impl From<chrono::format::ParseError> for DatastoreError {
    fn from(err: chrono::format::ParseError) -> Self {
        DatastoreError::TimestampParseError(err)
    }
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

    pub async fn archive_event_by_id(&self, event_id: String) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.archive_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.archive_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn escalate_event_by_id(&self, event_id: String) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.escalate_event_by_id(event_id).await,
            Datastore::SQLite(ds) => ds.escalate_event_by_id(event_id).await,
            _ => Err(DatastoreError::Unimplemented),
        }
    }

    pub async fn deescalate_event_by_id(&self, event_id: String) -> Result<(), DatastoreError> {
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
    ) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.comment_by_alert_group(alert_group, comment).await,
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
        event_id: String,
        comment: String,
    ) -> Result<(), DatastoreError> {
        match self {
            Datastore::Elastic(ds) => ds.comment_event_by_id(event_id, comment).await,
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
            //Datastore::SQLite(ds) => ds.agg(params).await,
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
}

#[derive(Default, Debug)]
pub struct HistogramParameters {
    pub min_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub max_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub interval: Option<HistogramInterval>,
    pub event_type: Option<String>,
    pub dns_type: Option<String>,
    pub address_filter: Option<String>,
    pub query_string: Option<String>,
    pub sensor_name: Option<String>,
}

#[derive(Debug, PartialEq)]
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
    pub min_timestamp: Option<DateTime>,
    pub agg: String,
    pub size: u64,
}

pub struct FlowHistogramParameters {
    pub mints: Option<DateTime>,
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
