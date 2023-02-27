// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::sqlite::{
    connection::{get_auto_vacuum, get_journal_mode, get_synchronous},
    init_event_db, ConnectionBuilder, SqliteExt,
};
use anyhow::Result;
use clap::CommandFactory;
use clap::{ArgMatches, Command, FromArgMatches, Parser, Subcommand};
use std::fs::File;
use tracing::info;

mod fts;

#[derive(Parser, Debug)]
#[command(name = "sqlite", about = "SQLite utilities")]
pub struct Args {
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
    Info(InfoArgs),
}

#[derive(Parser, Debug)]
struct InfoArgs {
    #[arg(long, short = 'D')]
    data_directory: Option<String>,
    filename: Option<String>,
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
        Commands::Dump { filename } => dump(filename),
        Commands::Load(args) => load(args),
        Commands::Fts(args) => fts::fts(args),
        Commands::Query { filename, sql } => query(filename, sql),
        Commands::Info(args) => info(args),
    }
}

fn info(args: &InfoArgs) -> Result<()> {
    let filename = if let Some(filename) = &args.filename {
        filename.to_string()
    } else if let Some(dir) = &args.data_directory {
        format!("{dir}/events.sqlite")
    } else {
        bail!("a filename or data directory must be specified");
    };

    let conn = ConnectionBuilder::filename(Some(&filename)).open(false)?;
    println!("Filename: {filename}");
    println!("Auto vacuum: {}", get_auto_vacuum(&conn)?);
    println!("Journal mode: {}", get_journal_mode(&conn)?);
    println!("Synchronous: {}", get_synchronous(&conn)?);
    println!("FTS enabled: {}", conn.has_table("fts")?);

    let min_rowid: i64 =
        conn.query_row_and_then("select min(rowid) from events", [], |row| row.get(0))?;
    let max_rowid: i64 =
        conn.query_row_and_then("select max(rowid) from events", [], |row| row.get(0))?;

    println!("Minimum rowid: {min_rowid}");
    println!("Maximum rowid: {max_rowid}");
    println!("Number of events (estimate): {}", max_rowid - min_rowid);

    let schema_version: i64 = conn.query_row_and_then(
        "select max(version) from refinery_schema_history",
        [],
        |row| row.get(0),
    )?;
    println!("Schema version: {schema_version}");

    let min_timestamp = conn.query_row_and_then(
        "select min(timestamp) from events",
        [],
        |row| -> anyhow::Result<time::OffsetDateTime> {
            let timestamp: i64 = row.get(0)?;
            Ok(time::OffsetDateTime::from_unix_timestamp_nanos(
                timestamp as i128,
            )?)
        },
    )?;

    let max_timestamp = conn.query_row_and_then(
        "select max(timestamp) from events",
        [],
        |row| -> anyhow::Result<time::OffsetDateTime> {
            let timestamp: i64 = row.get(0)?;
            Ok(time::OffsetDateTime::from_unix_timestamp_nanos(
                timestamp as i128,
            )?)
        },
    )?;

    println!("Oldest event: {min_timestamp}");
    println!("Latest event: {max_timestamp}");

    Ok(())
}

fn dump(filename: &str) -> Result<()> {
    let conn = ConnectionBuilder::filename(Some(filename)).open(false)?;
    let mut st = conn.prepare("select source from events order by timestamp")?;
    let mut rows = st.query([])?;
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        println!("{source}");
    }
    Ok(())
}

fn load(args: &LoadArgs) -> Result<()> {
    use std::io::{BufRead, BufReader};
    let input = File::open(&args.input)?;
    let reader = BufReader::new(input).lines();
    let mut conn = ConnectionBuilder::filename(Some(&args.filename)).open(true)?;
    init_event_db(&mut conn)?;
    let fts = conn.has_table("fts")?;
    info!("Loading events");
    let mut count = 0;
    let tx = conn.transaction()?;
    for line in reader {
        let line = line?;
        let mut eve: serde_json::Value = serde_json::from_str(&line)?;
        for statement in crate::sqlite::importer::prepare_sql(&mut eve, fts)? {
            let mut st = tx.prepare_cached(&statement.statement)?;
            st.execute(rusqlite::params_from_iter(&statement.params))?;
        }
        count += 1;
        if let Some(limit) = args.count {
            if count >= limit {
                break;
            }
        }
    }
    info!("Committing {count} events");
    tx.commit()?;
    Ok(())
}

fn query(filename: &str, sql: &str) -> Result<()> {
    let conn = ConnectionBuilder::filename(Some(filename)).open(false)?;
    let mut st = conn.prepare(sql)?;
    let mut rows = st.query([])?;
    let mut count = 0;
    while let Some(_row) = rows.next()? {
        count += 1;
    }
    println!("Query returned {count} rows");
    Ok(())
}
