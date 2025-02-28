// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use chrono::TimeZone;

use crate::elastic::{HistoryEntryBuilder, TAG_ARCHIVED, TAG_AUTO_ARCHIVED};
use crate::server::metrics::Metrics;
use crate::sqlite::configdb::{ConfigDb, EnabledWithValue};
use crate::{elastic::DateTime, prelude::*};

use super::client::SimpleIndexStats;
use super::{Client, ElasticEventRepo};

pub fn start(metrics: Arc<Metrics>, configdb: ConfigDb, repo: ElasticEventRepo) {
    tokio::spawn(run(metrics, configdb, repo));
}

async fn run(metrics: Arc<Metrics>, configdb: ConfigDb, repo: ElasticEventRepo) {
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

    loop {
        if let Err(err) = delete_indices(&configdb, &repo).await {
            warn!("Failed to delete indices: {:?}", err);
        }

        if let Err(err) = auto_archive(&metrics, &configdb, &repo).await {
            warn!("Failed to auto-archive events: {:?}", err);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn delete_indices(configdb: &ConfigDb, repo: &ElasticEventRepo) -> Result<()> {
    let config: Option<EnabledWithValue> = configdb.kv_get_config_as_t("config.retention").await?;
    let config = match config {
        Some(config) => config,
        None => return Ok(()),
    };
    if !config.enabled {
        return Ok(());
    }

    let indices = repo.client.get_index_stats(&repo.base_index).await?;
    let _count = do_delete_indices(&repo.client, indices, config.value, true).await?;

    Ok(())
}

pub(crate) async fn do_delete_indices(
    client: &Client,
    indices: Vec<SimpleIndexStats>,
    days: u64,
    force: bool,
) -> Result<i32> {
    let mut count = 0;
    for index in &indices {
        if let Some(date) = parse_index_date(&index.name) {
            let age = chrono::Utc::now().signed_duration_since(date).num_days();
            if age > days as i64 {
                info!("Deleting index older than {} days: {}", days, index.name);
                if force {
                    match client.delete_index(&index.name).await {
                        Ok(status) => {
                            if !status.is_success() {
                                error!("Failed to delete index: {}", status);
                            } else {
                                count += 1;
                            }
                        }
                        Err(err) => {
                            error!("Failed to delete index: {}", err);
                        }
                    }
                } else {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

async fn auto_archive(
    metrics: &Metrics,
    configdb: &ConfigDb,
    repo: &ElasticEventRepo,
) -> Result<()> {
    let config: Option<EnabledWithValue> =
        configdb.kv_get_config_as_t("config.autoarchive").await?;
    let config = match config {
        Some(config) => config,
        None => return Ok(()),
    };
    if !config.enabled {
        return Ok(());
    }

    let now = DateTime::now();
    let interval = chrono::Duration::days(config.value as i64).to_std()?;
    let then = now - interval;

    let mut filter = vec![];
    filter.push(json!({"exists": {"field": repo.map_field("event_type")}}));
    filter.push(json!({"term": {repo.map_field("event_type"): "alert"}}));
    filter.push(json!({
        "range": {
            repo.map_field("timestamp"): {
                "lt": then.to_elastic(),
            }
        }
    }));

    let query = json!({
        "bool": {
            "filter": filter,
            "must_not": {
                "terms": {
                    "tags": [TAG_ARCHIVED],
                }
            }
        }
    });

    let action = HistoryEntryBuilder::new_auto_archived().build();
    let tags = &[TAG_AUTO_ARCHIVED, TAG_ARCHIVED];
    let n = repo.add_tags_by_query(query, tags, &action).await?;
    metrics.incr_autoarchived_by_age(n);
    debug!("Auto-archived {} alerts", n);

    Ok(())
}

pub(crate) fn parse_index_date(name: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let re = regex::Regex::new(r"(\d{4})\.(\d{2})\.(\d{2})").unwrap();
    let caps = re.captures(name).unwrap();
    let year = caps.get(1).unwrap().as_str().parse::<i32>().unwrap();
    let month = caps.get(2).unwrap().as_str().parse::<u32>().unwrap();
    let day = caps.get(3).unwrap().as_str().parse::<u32>().unwrap();
    match chrono::Utc.with_ymd_and_hms(year, month, day, 0, 0, 0) {
        chrono::offset::LocalResult::Single(dt) => Some(dt),
        chrono::offset::LocalResult::Ambiguous(_, _) => None,
        chrono::offset::LocalResult::None => None,
    }
}
