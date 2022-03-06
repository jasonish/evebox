// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use clap::{Arg, Command};
use tracing::error;

pub fn command() -> clap::Command<'static> {
    Command::new("print")
        .about("EveBox Print Info")
        .arg(Arg::new("what").required(true))
}

const AGENT_CONFIG: &str = include_str!("../../examples/agent.yaml");

pub fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let what = args.value_of("what").unwrap();
    match what {
        "agent.yaml" => {
            print!("{}", AGENT_CONFIG)
        }
        _ => {
            error!("I don't know how to print {}", what);
        }
    }
    Ok(())
}
