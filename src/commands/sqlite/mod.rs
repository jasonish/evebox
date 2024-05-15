// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::{
    elastic::AlertQueryOptions,
    sqlite::{
        connection::init_event_db, eventrepo::SqliteEventRepo, has_table, info::Info,
        ConnectionBuilder,
    },
};
use anyhow::Result;
use clap::CommandFactory;
use clap::{ArgMatches, Command, FromArgMatches, Parser, Subcommand};
use futures::TryStreamExt;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::{fs::File, sync::Arc, time::Instant};
use tracing::info;

mod fts;

#[derive(Parser, Debug)]
#[command(name = "sqlite", about = "SQLite utilities")]
pub(crate) struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Dump EVE events from database
    Dump {
        /// Filename of SQLite database
        filename: String,
    },
    /// Load an EVE/JSON file
    Load(LoadArgs),
    /// Check, enable, disable FTS
    Fts(FtsArgs),
    /// Run an SQL query
    Query {
        #[arg(value_name = "DB_FILENAME")]
        filename: String,
        sql: String,
    },
    /// Information about an EveBox SQLite database
    Info(InfoArgs),
    /// Optimize an EveBox SQLite database
    Optimize(OptimizeArgs),
    /// Analyze an EveBox SQLite database
    Analyze { filename: String },
}

#[derive(Parser, Debug)]
struct ScratchArgs {
    filename: String,
}

#[derive(Parser, Debug)]
struct InfoArgs {
    #[arg(long, help = "Count events")]
    count: bool,
    #[arg(from_global, id = "data-directory")]
    data_directory: Option<String>,
    filename: Option<String>,
}

#[derive(Parser, Debug)]
struct OptimizeArgs {
    filename: String,
}

#[derive(Parser, Debug)]
struct FtsArgs {
    #[clap(subcommand)]
    command: FtsCommand,
}

#[derive(Subcommand, Debug)]
enum FtsCommand {
    /// Enable FTS
    Enable {
        #[arg(long)]
        force: bool,
        #[arg(value_name = "DB_FILENAME")]
        filename: String,
    },
    /// Disable FTS
    Disable {
        #[arg(long)]
        force: bool,
        #[arg(value_name = "DB_FILENAME")]
        filename: String,
    },
    /// Check FTS integrity
    Check {
        #[arg(value_name = "DB_FILENAME")]
        filename: String,
    },

    /// Optimize FTS data
    Optimize {
        #[arg(value_name = "DB_FILENAME")]
        filename: String,
    },
}

#[derive(Debug, Parser)]
struct LoadArgs {
    /// Limit the number of events to count
    #[arg(long, value_name = "COUNT")]
    count: Option<usize>,
    /// EVE file to load into database
    #[arg(short, long)]
    input: String,
    /// Filename of SQLite database
    filename: String,
}

pub fn command() -> Command {
    Args::command()
}

pub async fn main(args: &ArgMatches) -> anyhow::Result<()> {
    let args = Args::from_arg_matches(args)?;
    match &args.command {
        Commands::Dump { filename } => dump(filename).await,
        Commands::Load(args) => load(args).await,
        Commands::Fts(args) => fts::fts(args).await,
        Commands::Query { filename, sql } => query(filename, sql).await,
        Commands::Info(args) => info(args).await,
        Commands::Optimize(args) => optimize(args).await,
        Commands::Analyze { filename } => analyze(filename).await,
    }
}

async fn info(args: &InfoArgs) -> Result<()> {
    let filename = if let Some(filename) = &args.filename {
        filename.to_string()
    } else if let Some(dir) = &args.data_directory {
        format!("{dir}/events.sqlite")
    } else {
        bail!("a filename or data directory must be specified");
    };

    let connection_builder = ConnectionBuilder::filename(Some(&filename));
    let mut conn = connection_builder.open_connection(false).await?;
    let pool = connection_builder.open_pool(false).await?;

    let min_rowid: i64 = sqlx::query_scalar("SELECT MIN(rowid) FROM events")
        .fetch_optional(&mut conn)
        .await?
        .unwrap_or(0);
    let max_rowid: i64 = sqlx::query_scalar("SELECT MAX(rowid) FROM events")
        .fetch_optional(&mut conn)
        .await?
        .unwrap_or(0);

    let mut info = Info::new(&mut conn);

    println!("Filename: {filename}");
    println!("Auto vacuum: {}", info.get_auto_vacuum().await?);
    println!("Journal mode: {}", info.get_journal_mode().await?);
    println!("Synchronous: {}", info.get_synchronous().await?);
    println!("FTS enabled: {}", info.has_table("fts").await?);

    let page_size = info.pragma_i64("page_size").await?;
    let page_count = info.pragma_i64("page_count").await?;
    let freelist_count = info.pragma_i64("freelist_count").await?;

    println!("Page size: {page_size}");
    println!("Page count: {page_count}");
    println!("Free list count: {freelist_count}");
    println!(
        "Data size (page size * page count): {}",
        page_size * page_count
    );

    println!("Minimum rowid: {min_rowid}");
    println!("Maximum rowid: {max_rowid}");

    if args.count {
        let event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
            .fetch_one(&pool)
            .await?;
        println!("Number of events: {event_count}");
    } else {
        println!("Number of events (estimate): {}", max_rowid - min_rowid);
    }

    let schema_version = info.schema_version().await?;
    println!("Schema version: {schema_version}");

    let min_timestamp = sqlx::query("SELECT MIN(timestamp) FROM events")
        .try_map(|row: SqliteRow| {
            let timestamp: i64 = row.try_get(0)?;
            Ok(time::OffsetDateTime::from_unix_timestamp_nanos(
                timestamp as i128,
            ))
        })
        .fetch_optional(&pool)
        .await?
        .transpose()?;

    let max_timestamp = sqlx::query("SELECT MAX(timestamp) FROM events")
        .try_map(|row: SqliteRow| {
            let timestamp: i64 = row.try_get(0)?;
            Ok(time::OffsetDateTime::from_unix_timestamp_nanos(
                timestamp as i128,
            ))
        })
        .fetch_optional(&pool)
        .await?
        .transpose()?;

    println!("Oldest event: {min_timestamp:?}");
    println!("Latest event: {max_timestamp:?}");

    Ok(())
}

async fn dump(filename: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_connection(false)
        .await?;
    let mut rows = sqlx::query("SELECT source FROM events ORDER BY timestamp").fetch(&mut conn);
    while let Some(row) = rows.try_next().await? {
        let source: String = row.try_get(0)?;
        println!("{source}");
    }
    Ok(())
}

async fn load(args: &LoadArgs) -> Result<()> {
    use std::io::{BufRead, BufReader};
    let input = File::open(&args.input)?;
    let reader = BufReader::new(input).lines();
    let connection_builder = ConnectionBuilder::filename(Some(&args.filename));
    let mut conn = connection_builder.open_connection(true).await?;
    init_event_db(&mut conn).await?;
    let fts = has_table(&mut conn, "fts").await?;
    info!("Loading events");
    let mut count = 0;

    let conn = Arc::new(tokio::sync::Mutex::new(conn));
    let mut importer = crate::sqlite::importer::SqliteEventSink::new(conn, fts);

    // This could be improved if the importer exposed some more inner
    // details so the caller could control the transaction.
    for line in reader {
        let line = line?;
        let eve: serde_json::Value = serde_json::from_str(&line)?;
        importer.submit(eve).await?;
        count += 1;
        if let Some(limit) = args.count {
            if count >= limit {
                break;
            }
        }
        if count > 0 && count % 1000 == 0 {
            importer.commit().await?;
        }
    }
    info!("Committing {count} events");
    importer.commit().await?;
    Ok(())
}

async fn query(filename: &str, sql: &str) -> Result<()> {
    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_connection(false)
        .await?;
    let mut count = 0;
    let timer = Instant::now();
    let mut rows = sqlx::query(sql).fetch(&mut conn);
    while let Some(_row) = rows.try_next().await? {
        count += 1;
    }
    println!("Query returned {count} rows in {:?}", timer.elapsed());
    Ok(())
}

async fn optimize(args: &OptimizeArgs) -> Result<()> {
    let conn = ConnectionBuilder::filename(Some(&args.filename))
        .open_connection(false)
        .await?;
    let conn = Arc::new(tokio::sync::Mutex::new(conn));
    let pool = crate::sqlite::connection::open_pool(Some(&args.filename), false).await?;
    let repo = SqliteEventRepo::new(conn, pool.clone(), false);

    info!("Running inbox style query");
    let gte = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    repo.alerts(AlertQueryOptions {
        timestamp_gte: Some(gte),
        query_string: None,
        tags: vec![],
        sensor: None,
    })
    .await
    .map_err(|err| anyhow::anyhow!(format!("{}", err)))?;

    info!("Optimizing");
    let mut conn = crate::sqlite::connection::open_connection(Some(&args.filename), false).await?;
    sqlx::query("PRAGMA analysis_limit = 400")
        .execute(&mut conn)
        .await?;
    sqlx::query("PRAGMA optmize").execute(&mut conn).await?;
    info!("Done. If EveBox is running, it is recommended to restart it.");
    Ok(())
}

async fn analyze(filename: &str) -> Result<()> {
    match inquire::Confirm::new("This could take a while, do you want to continue?")
        .with_default(true)
        .prompt()
    {
        Err(_) | Ok(false) => return Ok(()),
        Ok(true) => {}
    }

    let mut conn = ConnectionBuilder::filename(Some(filename))
        .open_connection(false)
        .await?;
    sqlx::query("analyze").execute(&mut conn).await?;
    info!("Done");
    Ok(())
}
