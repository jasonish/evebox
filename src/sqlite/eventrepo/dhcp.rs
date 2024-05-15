// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::SqliteEventRepo;
use crate::eventrepo::DatastoreError;
use futures::TryStreamExt;
use sqlx::sqlite::SqliteArguments;
use sqlx::Arguments;
use sqlx::Row;
use time::OffsetDateTime;

impl SqliteEventRepo {
    pub async fn dhcp(
        &self,
        earliest: Option<OffsetDateTime>,
        dhcp_type: &str,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut wheres = vec![
            "json_extract(events.source, '$.event_type') = 'dhcp'".to_string(),
            format!("json_extract(events.source, '$.dhcp.dhcp_type') = '{dhcp_type}'"),
        ];
        let mut params = SqliteArguments::default();

        if let Some(earliest) = earliest {
            params.add(earliest.unix_timestamp_nanos() as i64);
            wheres.push("timestamp >= ?".to_string())
        }

        if let Some(sensor) = &sensor {
            wheres.push("json_extract(events.source, '$.host') = ?".to_string());
            params.add(sensor.to_string());
        }

        let sql = r#"
            select t1.source
            from events t1
              join(
                select max(timestamp) as timestamp,
                       json_extract(events.source, '$.dhcp.client_mac') as dhcp_client_mac
                from events
                where %where%
                group by json_extract(events.source, '$.dhcp.client_mac')
              ) t2
            on
              t1.timestamp = t2.timestamp
              and json_extract(t1.source, '$.dhcp.client_mac') = t2.dhcp_client_mac
            where json_extract(t1.source, '$.event_type') = 'dhcp'
        "#;

        let sql = sql.replace("%where%", &wheres.join(" and "));
        let mut rows = sqlx::query_with(&sql, params).fetch(&self.pool);
        let mut events = vec![];
        while let Some(row) = rows.try_next().await? {
            let event: String = row.try_get(0)?;
            let event: serde_json::Value = serde_json::from_str(&event)?;
            events.push(event);
        }

        Ok(events)
    }

    pub async fn dhcp_request(
        &self,
        earliest: Option<OffsetDateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        self.dhcp(earliest, "request", sensor).await
    }

    pub async fn dhcp_ack(
        &self,
        earliest: Option<OffsetDateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        self.dhcp(earliest, "ack", sensor).await
    }
}
