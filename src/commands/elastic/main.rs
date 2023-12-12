// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use crate::commands::elastic::info;
use clap::{ArgAction, Command};

pub fn main_options() -> Command {
    let info = Command::new("info");
    Command::new("elastic")
        .about("Elasticsearch utilities")
        .arg(
            clap::Arg::new("elasticsearch")
                .short('e')
                .long("elasticsearch")
                .action(ArgAction::Set)
                .default_value("http://localhost:9200")
                .hide_default_value(true)
                .help("Elastic Search URL")
                .global(true),
        )
        .subcommand(info)
        .subcommand_required(true)
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("info", args)) => info::main(args).await?,
        _ => unreachable!(),
    }
    Ok(())
}
