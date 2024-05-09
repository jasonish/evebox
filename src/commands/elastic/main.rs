// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::commands::elastic::info;
use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};

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
}

pub fn main_options() -> Command {
    Args::command()
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let args = Args::from_arg_matches(args)?;
    match args.commands {
        Commands::Info(args) => info::main(args).await?,
        Commands::SetFieldLimit(args) => set_field_limit::main(args).await?,
    }
    Ok(())
}
