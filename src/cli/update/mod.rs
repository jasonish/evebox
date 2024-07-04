// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::cli::prelude::*;

pub mod ja4db;

#[derive(Debug, Parser)]
#[command(name = "update")]
pub struct Args {
    #[command(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Update JA4db.
    Ja4db(ja4db::Args),
}

pub fn args() -> Command {
    Args::command()
}

pub async fn main(args: &ArgMatches) -> Result<()> {
    let args = Args::from_arg_matches(args)?;

    match args.subcommand {
        Commands::Ja4db(args) => ja4db::main(args).await,
    }
}
