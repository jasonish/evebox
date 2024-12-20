// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use sqlx::Connection;
use tracing::warn;

use crate::{elastic::HistoryEntryBuilder, error::AppError, server::session::Session};

use super::SqliteEventRepo;

impl SqliteEventRepo {
    pub async fn comment_event_by_id(
        &self,
        event_id: &str,
        comment: String,
        session: Arc<Session>,
    ) -> Result<(), AppError> {
        let event_id: i64 = event_id.parse()?;
        let action = HistoryEntryBuilder::new_comment()
            .username(session.username.clone())
            .comment(comment)
            .build();
        let mut conn = self.writer.lock().await;
        let mut tx = conn.begin().await?;

        let sql = r#"
            UPDATE events
            SET history = json_insert(history, '$[#]', json(?))
            WHERE rowid = ?"#;

        let n = sqlx::query(sql)
            .bind(action.to_json())
            .bind(event_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        tx.commit().await?;

        if n == 0 {
            warn!("Archive by event ID request did not update any events");
            Err(AppError::EventNotFound)
        } else {
            Ok(())
        }
    }
}
