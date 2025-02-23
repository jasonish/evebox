// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use chrono::TimeZone;
use clap::Parser;

use crate::{elastic::Client, prelude::*};

#[derive(Debug, Parser)]
pub(crate) struct DeleteArgs {
    #[clap(flatten)]
    pub elastic: super::main::ElasticOptions,

    /// Force delete, required to actually delete.
    #[clap(short, long)]
    force: bool,

    /// Base index.
    index: String,

    /// Delete indices older than this many days.
    days: u64,
}

pub(super) async fn delete(args: DeleteArgs) -> Result<()> {
    let mut client = Client::new(&args.elastic.elasticsearch);

    if args.elastic.username.is_some() {
        client.username.clone_from(&args.elastic.username);
    }

    if args.elastic.password.is_some() {
        client.password.clone_from(&args.elastic.password);
    }

    let indices = client.get_index_stats(&args.index).await?;
    let mut count = 0;
    let now = chrono::Utc::now();
    for index in &indices {
        if let Some(date) = parse_date(&index.name) {
            let age = now.signed_duration_since(date).num_days();
            if age > args.days as i64 {
                println!("Deleting index: {} (dry-run={})", &index.name, !args.force);
                if args.force {
                    match client.delete_index(&index.name).await {
                        Ok(status) => {
                            if !status.is_success() {
                                error!("Failed to delete index: {}", status);
                            }
                        }
                        Err(err) => {
                            error!("Failed to delete index: {}", err);
                        }
                    }
                }
                count += 1;
            }
        } else {
            error!("Failed to parse date from index name: {}", &index.name);
        }
    }

    if args.force {
        info!("Deleted {} indices.", count);
    } else {
        warn!(
            "Would have deleted {} indices, add --force to actually delete.",
            count
        );
    }

    Ok(())
}

fn parse_date(name: &str) -> Option<chrono::DateTime<chrono::Utc>> {
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
