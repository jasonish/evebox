// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use crate::commands::elastic::info;
use clap::{ArgAction, Command};

pub fn main_options() -> Command {
    let info = Command::new("info");
    Command::new("elastic")
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
