// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use crate::commands::elastic::info;
use clap::Command;

pub fn main_options() -> Command<'static> {
    let info = Command::new("info");
    Command::new("elastic")
        .arg(
            clap::Arg::new("elasticsearch")
                .short('e')
                .long("elasticsearch")
                .takes_value(true)
                .default_value("http://localhost:9200")
                .hide_default_value(true)
                .help("Elastic Search URL")
                .global(true),
        )
        .subcommand(info)
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("info", args)) => info::main(args).await?,
        _ => unreachable!(),
    }
    Ok(())
}
