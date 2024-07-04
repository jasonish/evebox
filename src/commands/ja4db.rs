// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use serde::Deserialize;
use sqlx::SqliteConnection;

#[derive(Debug, Deserialize)]
struct Entry {
    ja4_fingerprint: Option<String>,
}

pub(crate) async fn updatedb(conn: &mut SqliteConnection) -> Result<i32> {
    let url = "https://ja4db.com/api/download/";
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    let objects: Vec<serde_json::Value> = serde_json::from_str(&body)?;
    let mut n = 0;
    for entry in &objects {
        let parsed: Entry = serde_json::from_value(entry.clone())?;
        if let Some(fp) = parsed.ja4_fingerprint {
            let sql = r#"INSERT INTO ja4db (fingerprint, data) VALUES (?, ?)
                ON CONFLICT(fingerprint) DO UPDATE SET data = ?
                "#;
            sqlx::query(sql)
                .bind(&fp)
                .bind(entry)
                .bind(entry)
                .execute(&mut *conn)
                .await?;
            n += 1;
        }
    }

    Ok(n)
}
