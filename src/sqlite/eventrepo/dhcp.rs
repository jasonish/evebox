// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use super::SqliteEventRepo;
use crate::LOG_QUERY_PLAN;
use crate::datetime::DateTime;
use crate::sqlite::log_query_plan;

impl SqliteEventRepo {
    pub async fn dhcp(
        &self,
        earliest: Option<DateTime>,
        dhcp_type: &str,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut wheres = vec![
            "json_extract(events.source, '$.event_type') = 'dhcp'".to_string(),
            format!("json_extract(events.source, '$.dhcp.dhcp_type') = '{dhcp_type}'"),
        ];
        let mut params = SqliteArguments::default();

        if let Some(earliest) = earliest {
            params.push(earliest.to_nanos())?;
            wheres.push("timestamp >= ?".to_string())
        }

        if let Some(sensor) = &sensor {
            if sensor == "(no-name)" {
                wheres.push("json_extract(events.source, '$.host') IS NULL".to_string());
            } else {
                wheres.push("json_extract(events.source, '$.host') = ?".to_string());
                params.push(sensor.to_string())?;
            }
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

        if *LOG_QUERY_PLAN {
            log_query_plan(&self.pool, &sql, &params).await;
        }

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
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        self.dhcp(earliest, "request", sensor).await
    }

    pub async fn dhcp_ack(
        &self,
        earliest: Option<DateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>> {
        self.dhcp(earliest, "ack", sensor).await
    }
}
