// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::cli::prelude::*;

mod extract;
mod purge;

#[derive(Debug, Parser)]
#[command(name = "pcap", about = "Full packet capture tools")]
pub struct Args {
    // Hide the global data-directory option.
    #[arg(hide = true, global = true, short = 'D', long, name = "data-directory")]
    data_directory: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Extract packets from PCAP spool files
    Extract(extract::ExtractArgs),

    /// Purge old PCAP files from a spool directory
    Purge(purge::PurgeArgs),
}

pub fn command() -> Command {
    Args::command()
}

pub async fn main(args: &ArgMatches) -> Result<()> {
    let args = Args::from_arg_matches(args)?;

    match args.command {
        Commands::Extract(args) => extract::main(args),
        Commands::Purge(args) => purge::main(args).await,
    }
}
