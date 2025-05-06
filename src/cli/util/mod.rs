// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::cli::prelude::*;

pub(crate) mod eve2pcap;

#[derive(Debug, Parser)]
#[command(name = "util", about = "Extra utilities")]
pub struct Args {
    #[command(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Convert Suricata EVE JSON to PCAP
    Eve2pcap(eve2pcap::Args),
}

pub fn args() -> Command {
    Args::command()
}

pub async fn main(args: &ArgMatches) -> Result<()> {
    let args = Args::from_arg_matches(args)?;

    match args.subcommand {
        Commands::Eve2pcap(args) => eve2pcap::main(args).await,
    }
}
