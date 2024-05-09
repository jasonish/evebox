// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use clap::{CommandFactory, Parser, Subcommand};

use self::users::UsersCommand;

pub mod users;

#[derive(Parser, Debug)]
#[command(name = "config", about = "Configuration commands")]
pub(crate) struct Args {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    Users(UsersCommand),
}

pub fn config_subcommand() -> clap::Command {
    Args::command()
}

pub fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("users", args)) => users::main(args),
        _ => Err(anyhow!("no subcommand provided")),
    }
}
