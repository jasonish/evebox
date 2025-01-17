// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;
use crate::sqlite::prelude::*;

use crate::datetime::DateTime;
use crate::sqlite::eventrepo::SqliteEventRepo;

impl SqliteEventRepo {
    pub(crate) async fn dns_reverse_lookup(
        &self,
        before: Option<DateTime>,
        sensor: Option<String>,
        src_ip: String,
        dest_ip: String,
    ) -> Result<serde_json::Value> {
        // If before is None, set to now.
        let before = before.unwrap_or_else(DateTime::now);

        // Set after to 24 hours before before.
        let after: DateTime = (before.datetime - chrono::Duration::hours(1)).into();

        let mut builder = crate::sqlite::builder::EventQueryBuilder::new(self.fts().await);
        builder.select("DISTINCT COALESCE(events.source->>'dns'->>'queries'->>0->>'rrname', events.source->>'dns'->>'rrname')");
        builder.from("events");
        builder.from("json_each(events.source, '$.dns.answers') AS answers");

        builder.timestamp_lte(&before).unwrap();
        builder.timestamp_gte(&after).unwrap();

        builder
            .push_where("json_extract(events.source, '$.event_type') = ?")
            .push_arg("dns")
            .unwrap();

        if let Some(host) = sensor {
            builder.wherejs("host", "=", host).unwrap();
        }

        builder.push_where(
            r#"(
        json_extract(events.source, '$.dest_ip') = ? OR 
        json_extract(events.source, '$.dest_ip') = ? OR
        json_extract(events.source, '$.src_ip') = ? OR
        json_extract(events.source, '$.src_ip') = ?)
    "#,
        );
        builder.push_arg(&src_ip).unwrap();
        builder.push_arg(&dest_ip).unwrap();
        builder.push_arg(&src_ip).unwrap();
        builder.push_arg(&dest_ip).unwrap();

        builder.push_where("answers.value->>'rdata' = ?");
        builder.push_arg(&src_ip).unwrap();

        // Limit to responses.
        builder.push_where(
        "(events.source->>'dns'->>'type' = 'response' OR events.source->>'dns'->>'type' = 'answer')",
    );

        builder.order_by("events.timestamp", "DESC");

        let (sql, params) = builder.build().unwrap();
        let mut tx = self.pool.begin().await.unwrap();
        set_tx_timeout(&mut tx, std::time::Duration::from_secs(3)).await?;

        let mut rrnames = vec![];

        let mut rows = sqlx::query_scalar_with(&sql, params).fetch(&mut *tx);
        loop {
            match rows.try_next().await {
                Ok(None) => {
                    break;
                }
                Ok(Some(row)) => {
                    let rrname: String = row;
                    rrnames.push(rrname);
                }
                Err(_err) => {
                    break;
                }
            }
        }

        Ok(json!({
            "rrnames": rrnames,
        }))
    }
}

async fn set_tx_timeout(
    tx: &mut SqliteConnection,
    duration: std::time::Duration,
) -> Result<(), sqlx::error::Error> {
    let now = std::time::Instant::now();
    tx.lock_handle()
        .await?
        .set_progress_handler(100, move || now.elapsed() <= duration);
    Ok(())
}
