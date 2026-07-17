// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::FromArgMatches;
use clap::Parser;
use clap::Subcommand;

use super::users::open_config_repo;

#[derive(Parser, Debug)]
#[command(name = "agents", about = "Configure agent keys")]
pub(crate) struct AgentsCommand {
    #[command(subcommand)]
    command: AgentsCommands,
}

#[derive(Debug, Subcommand)]
enum AgentsCommands {
    /// Add a new agent key
    Add {
        /// Agent name, normally the agent's `agent-id`
        name: String,
        #[arg(from_global, id = "config-directory")]
        config_directory: Option<String>,
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
    /// List agent keys
    #[command(alias = "ls")]
    List {
        /// Include the key values in the listing
        #[arg(long)]
        keys: bool,
        #[arg(from_global, id = "config-directory")]
        config_directory: Option<String>,
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
    /// Remove an agent key
    Rm {
        name: String,
        #[arg(from_global, id = "config-directory")]
        config_directory: Option<String>,
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
}

pub(crate) async fn main(args: &clap::ArgMatches) -> Result<()> {
    let args = AgentsCommands::from_arg_matches(args)?;
    match args {
        AgentsCommands::Add {
            name,
            config_directory,
            data_directory,
        } => add(name, config_directory, data_directory).await,
        AgentsCommands::List {
            keys,
            config_directory,
            data_directory,
        } => list(keys, config_directory, data_directory).await,
        AgentsCommands::Rm {
            name,
            config_directory,
            data_directory,
        } => remove(name, config_directory, data_directory).await,
    }
}

async fn add(
    name: String,
    config_directory: Option<String>,
    data_directory: Option<String>,
) -> Result<()> {
    let repo = open_config_repo(config_directory.as_deref(), data_directory.as_deref()).await?;
    let added = repo.add_agent_key(&name).await?;
    println!("Agent key added: name={:?}", added.name);
    println!("Key: {}", added.key);
    println!("Set this as server.key in the agent's agent.yaml (or EVEBOX_SERVER_KEY).");
    Ok(())
}

async fn list(
    keys: bool,
    config_directory: Option<String>,
    data_directory: Option<String>,
) -> Result<()> {
    let repo = open_config_repo(config_directory.as_deref(), data_directory.as_deref()).await?;
    for row in repo.list_agent_keys().await? {
        let mut value = serde_json::to_value(&row)?;
        if !keys {
            // Keys are re-showable on request (--keys) but kept out of the
            // default listing.
            value.as_object_mut().unwrap().remove("key");
        }
        println!("{}", serde_json::to_string(&value)?);
    }
    Ok(())
}

async fn remove(
    name: String,
    config_directory: Option<String>,
    data_directory: Option<String>,
) -> Result<()> {
    let repo = open_config_repo(config_directory.as_deref(), data_directory.as_deref()).await?;
    if !repo.remove_agent_key(&name).await? {
        return Err(anyhow!("no agent key named {name:?}"));
    }
    println!("Agent key removed: name={name:?}");
    println!(
        "Note: keys are checked when an agent connects; restart the EveBox server \
         to disconnect an agent that is currently connected with this key."
    );
    Ok(())
}
