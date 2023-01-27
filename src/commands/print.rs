// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use clap::Command;

pub fn command() -> clap::Command<'static> {
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
