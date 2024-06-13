// SPDX-FileCopyrightText: (C) 2022 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use clap::Command;

pub fn command() -> clap::Command {
    Command::new("print")
        .about("EveBox Print Info")
        .subcommand(Command::new("agent.yaml"))
}

const AGENT_CONFIG: &str = include_str!("../../examples/agent.yaml");

pub fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    match args.subcommand() {
        Some(("agent.yaml", _)) => {
            print!("{AGENT_CONFIG}");
        }
        _ => {
            unreachable!()
        }
    }
    Ok(())
}
