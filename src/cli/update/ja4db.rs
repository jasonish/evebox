// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use serde::Deserialize;
use tracing::info;

use crate::cli::prelude::*;

#[derive(Debug, Parser)]
pub(super) struct Args {
    #[arg(from_global, id = "data-directory")]
    data_directory: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    ja4_fingerprint: Option<String>,
}

pub(super) async fn main(args: Args) -> Result<()> {
    let dd = crate::config::get_data_directory(args.data_directory.as_deref());
    let mut configdb = crate::sqlite::configrepo::open_connection_in_directory(&dd).await?;

    let url = "https://ja4db.com/api/download/";
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    let objects: Vec<serde_json::Value> = serde_json::from_str(&body)?;
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
                .execute(&mut configdb)
                .await?;
        }
    }
    info!("Downloading {} entries", objects.len());

    Ok(())
}
