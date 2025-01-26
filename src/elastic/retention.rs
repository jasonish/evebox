// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::elastic::{HistoryEntryBuilder, TAG_ARCHIVED, TAG_AUTO_ARCHIVED};
use crate::server::metrics::Metrics;
use crate::sqlite::configdb::{AutoArchiveConfig, ConfigDb};
use crate::{elastic::DateTime, prelude::*};

use super::ElasticEventRepo;

pub fn start(metrics: Arc<Metrics>, configdb: ConfigDb, repo: ElasticEventRepo) {
    tokio::spawn(run(metrics, configdb, repo));
}

async fn run(metrics: Arc<Metrics>, configdb: ConfigDb, repo: ElasticEventRepo) {
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

    loop {
        if let Err(err) = auto_archive(&metrics, &configdb, &repo).await {
            warn!("Failed to auto-archive events: {:?}", err);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn auto_archive(
    metrics: &Metrics,
    configdb: &ConfigDb,
    repo: &ElasticEventRepo,
) -> Result<()> {
    let config: Option<AutoArchiveConfig> =
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
