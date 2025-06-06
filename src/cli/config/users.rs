// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::FromArgMatches;
use clap::Parser;
use clap::Subcommand;
use tracing::info;

use crate::sqlite::configdb;
use crate::sqlite::configdb::ConfigDb;

#[derive(Parser, Debug)]
#[command(name = "users", about = "Configure users")]
pub(crate) struct UsersCommand {
    #[command(subcommand)]
    command: UsersCommands,
}

#[derive(Debug, Subcommand)]
enum UsersCommands {
    /// Add a new user
    Add(AddArgs),
    /// Remove an existing user
    Rm {
        username: String,
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
    /// List users
    #[command(alias = "ls")]
    List {
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
    /// Change password
    Passwd {
        username: String,
        #[arg(from_global, id = "data-directory")]
        data_directory: Option<String>,
    },
}

#[derive(Parser, Debug)]
struct AddArgs {
    #[arg(long, short)]
    username: Option<String>,
    #[arg(long, short)]
    password: Option<String>,

    #[arg(from_global, id = "data-directory")]
    data_directory: Option<String>,
}

pub(crate) async fn main(args: &clap::ArgMatches) -> Result<()> {
    let args = UsersCommands::from_arg_matches(args)?;
    match args {
        UsersCommands::Add(args) => add(args).await,
        UsersCommands::List { data_directory } => list(data_directory).await,
        UsersCommands::Rm {
            username,
            data_directory,
        } => remove(username, data_directory).await,
        UsersCommands::Passwd {
            username,
            data_directory,
        } => password(username, data_directory).await,
    }
}

async fn open_config_repo<P: AsRef<Path>>(data_directory: Option<P>) -> Result<ConfigDb> {
    let data_directory = data_directory
        .map(|p| PathBuf::from(p.as_ref()))
        .or_else(crate::path::data_directory);
    let data_directory = match data_directory {
        Some(data_directory) => data_directory,
        None => {
            return Err(anyhow!("--data-directory required"));
        }
    };
    info!("Using data directory {}", data_directory.display());
    let filename = data_directory.join("config.sqlite");
    let config_repo = configdb::open(Some(&filename)).await?;
    Ok(config_repo)
}

async fn list(dir: Option<String>) -> Result<()> {
    let repo = open_config_repo(dir.as_deref()).await?;
    let users = repo.get_users().await?;
    for user in users {
        println!("{}", serde_json::to_string(&user).unwrap());
    }
    Ok(())
}

async fn add(args: AddArgs) -> Result<()> {
    let repo = open_config_repo(args.data_directory.as_deref()).await?;

    let username = if let Some(username) = args.username {
        username.to_string()
    } else {
        inquire::Text::new("Username:")
            .with_validator(inquire::required!())
            .prompt()?
    };
    if username.is_empty() {
        return Err(anyhow!("empty username not allowed"));
    }

    let password = if let Some(password) = args.password {
        password.to_string()
    } else {
        inquire::Password::new("Password:")
            .with_display_toggle_enabled()
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()?
    };

    repo.add_user(&username, &password).await?;
    println!("User added: username=\"{username}\"");

    Ok(())
}

async fn remove(username: String, dir: Option<String>) -> Result<()> {
    let repo = open_config_repo(dir.as_deref()).await?;
    if repo.remove_user(&username).await? == 0 {
        return Err(anyhow!("user does not exist"));
    }
    println!("User removed: username=\"{username}\"");
    Ok(())
}

async fn password(username: String, data_directory: Option<String>) -> Result<()> {
    let repo = open_config_repo(data_directory).await?;
    let user = repo.get_user_by_name(&username).await?;
    let password = inquire::Password::new("Password:")
        .with_display_toggle_enabled()
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_validator(inquire::required!())
        .prompt()?;
    if repo.update_password_by_id(&user.uuid, &password).await? {
        println!("Password has been updated.");
        Ok(())
    } else {
        Err(anyhow!("Failed to update password, user does not exist"))
    }
}
