// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use std::io::{stdin, stdout, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::CommandFactory;
use clap::FromArgMatches;
use clap::Parser;
use clap::Subcommand;

use crate::sqlite::configrepo::ConfigRepo;

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

pub(crate) fn command() -> clap::Command {
    UsersCommand::command()
}

pub(crate) fn main(args: &clap::ArgMatches) -> Result<()> {
    let args = UsersCommands::from_arg_matches(args)?;
    match args {
        UsersCommands::Add(args) => add(args),
        UsersCommands::List { data_directory } => list(data_directory),
        UsersCommands::Rm {
            username,
            data_directory,
        } => remove(username, data_directory),
        UsersCommands::Passwd {
            username,
            data_directory,
        } => password(username, data_directory),
    }
}

fn open_config_repo(data_directory: Option<&str>) -> Result<ConfigRepo> {
    if data_directory.is_none() {
        return Err(anyhow!("--data-directory required"));
    }
    let data_directory = data_directory.unwrap();
    let filename = PathBuf::from(data_directory).join("config.sqlite");
    let repo = ConfigRepo::new(Some(&filename))?;
    Ok(repo)
}

fn list(dir: Option<String>) -> Result<()> {
    let repo = open_config_repo(dir.as_deref())?;
    let users = repo.get_users()?;
    for user in users {
        println!("{}", serde_json::to_string(&user).unwrap());
    }
    Ok(())
}

fn get_input(prompt: &str) -> Result<String> {
    print!("{prompt}");
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn add(args: AddArgs) -> Result<()> {
    let repo = open_config_repo(args.data_directory.as_deref())?;

    let username = if let Some(username) = args.username {
        username.to_string()
    } else {
        get_input("Username: ")?
    };
    if username.is_empty() {
        return Err(anyhow!("empty username not allowed"));
    }

    let password = if let Some(password) = args.password {
        password.to_string()
    } else {
        // Yes, we're allowing empty passwords.
        let password = rpassword::read_password_from_tty(Some("Password: "))?;
        let confirm = rpassword::read_password_from_tty(Some("Confirm password: "))?;
        if password != confirm {
            return Err(anyhow!("passwords to not match"));
        }
        password
    };

    repo.add_user(&username, &password)?;
    println!("User added: username=\"{username}\"");

    Ok(())
}

fn remove(username: String, dir: Option<String>) -> Result<()> {
    let repo = open_config_repo(dir.as_deref())?;
    if repo.remove_user(&username)? == 0 {
        return Err(anyhow!("user does not exist"));
    }
    println!("User removed: username=\"{username}\"");
    Ok(())
}

fn password(username: String, dir: Option<String>) -> Result<()> {
    let repo = open_config_repo(dir.as_deref())?;
    let user = repo.get_user_by_name(&username)?;
    let password = rpassword::read_password_from_tty(Some("Password: "))?;
    if password.is_empty() {
        return Err(anyhow!("empty password not allowed"));
    }
    let confirm = rpassword::read_password_from_tty(Some("Confirm password: "))?;
    if password != confirm {
        return Err(anyhow!("passwords to not match"));
    }
    if repo.update_password_by_id(&user.uuid, &password)? {
        println!("Password has been updated.");
        Ok(())
    } else {
        Err(anyhow!("Failed to update password, user does not exist"))
    }
}
