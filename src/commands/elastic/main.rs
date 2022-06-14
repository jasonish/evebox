// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use crate::commands::elastic::info;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(name = "elastic")]
pub struct Options {
    #[clap(long, short = 'e', default_value = "http://127.0.0.1:9200")]
    elasticsearch: String,

    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Subcommand)]
pub enum Subcommands {
    Info,
}

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("info", _)) => info::main(args).await?,
        _ => unreachable!(),
    }
    Ok(())
}
