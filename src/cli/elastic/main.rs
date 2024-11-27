// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};
use tracing::info;

use super::set_field_limit;

#[derive(Parser, Debug)]
#[command(name = "elastic", about = "Elasticsearch utilities")]
pub(crate) struct Args {
    #[clap(flatten)]
    pub(crate) options: ElasticOptions,

    #[command(subcommand)]
    commands: Commands,
}

#[derive(Clone, Debug, Parser, Default)]
pub(crate) struct ElasticOptions {
    /// Elasticsearch URL
    #[clap(
        short,
        long,
        global = true,
        default_value = "http://localhost:9200",
        env = "EVEBOX_ELASTICSEARCH_URL",
        hide_env = true
    )]
    pub(crate) elasticsearch: String,

    /// Elasticsearch username.
    #[clap(short, long, global = true)]
    pub(crate) username: Option<String>,

    /// Elasticsearch password.
    #[clap(short, long, global = true)]
    pub(crate) password: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Display infomratiot about the Elasticsearch server
    Info(ElasticOptions),

    /// Set the field limit
    SetFieldLimit(set_field_limit::Args),

    /// Get the field limit.
    GetFieldLimit,
}

pub fn main_options() -> Command {
    Args::command()
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let args = Args::from_arg_matches(args)?;
    match args.commands {
        Commands::Info(args) => crate::cli::elastic::info::main(args).await?,
        Commands::SetFieldLimit(args) => set_field_limit::main(args).await?,
        Commands::GetFieldLimit => get_field_limit(&args).await?,
    }
    Ok(())
}

async fn get_field_limit(args: &Args) -> Result<()> {
    let mut client = crate::elastic::client::ClientBuilder::new(&args.options.elasticsearch)
        .disable_certificate_validation(true);
    if let Some(username) = &args.options.username {
        client = client.with_username(username);
    }
    if let Some(password) = &args.options.password {
        client = client.with_password(password);
    }
    let client = client.build();

    for index in client.get_indices_pattern("*").await? {
        // Only look at indices that match the name-YYYY.MM.DD
        // pattern.
        if regex::Regex::new(r"^\w+-\d{4}\.\d{2}\.\d{2}$")?
            .find(&index.index)
            .is_none()
        {
            continue;
        }

        let settings = client.get_index_settings(&index.index).await?;
        if let Some(map) = settings.as_object() {
            for (index, settings) in map {
                let limit = &settings["settings"]["index"]["mapping"]["total_fields"]["limit"];
                match limit {
                    serde_json::Value::Number(limit) => {
                        info!("{}: {}", index, limit);
                    }
                    serde_json::Value::String(limit) => {
                        info!("{}: {}", index, limit);
                    }
                    _ => {
                        info!("{}: default (likely 1000)", index);
                    }
                }
            }
        }
    }

    Ok(())
}
