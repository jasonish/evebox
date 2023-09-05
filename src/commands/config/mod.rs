// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

pub mod users;

pub fn config_subcommand() -> clap::Command {
    clap::Command::new("config").subcommand(users::command())
}

pub fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("users", args)) => users::main(args),
        _ => Err(anyhow!("no subcommand provided")),
    }
}
