// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::io::{stdin, stdout, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Arg;
use clap::SubCommand;

use crate::sqlite::configrepo::ConfigRepo;

pub fn users_subcommand<'a, 'b>() -> clap::App<'a, 'b> {
    clap::SubCommand::with_name("users")
        .subcommand(
            SubCommand::with_name("list")
                .alias("ls")
                .about("List users"),
        )
        .subcommand(
            SubCommand::with_name("add")
                .about("Add user")
                .arg(
                    Arg::with_name("username")
                        .long("username")
                        .short("u")
                        .value_name("USERNAME")
                        .help("Username"),
                )
                .arg(
                    Arg::with_name("password")
                        .long("password")
                        .short("p")
                        .value_name("PASSWORD")
                        .help("Password"),
                ),
        )
        .subcommand(
            SubCommand::with_name("rm")
                .about("Remove user")
                .arg(Arg::with_name("username").required(true)),
        )
        .subcommand(
            SubCommand::with_name("passwd")
                .alias("password")
                .about("Change password for user")
                .arg(Arg::with_name("username").required(true)),
        )
}

pub fn main(args: &clap::ArgMatches) -> Result<()> {
    match args.subcommand() {
        ("list", Some(args)) => list(&args),
        ("add", Some(args)) => add(&args),
        ("rm", Some(args)) => remove(&args),
        ("passwd", Some(args)) => password(&args),
        _ => {
            return Err(anyhow!("config users: no subcommand provided"));
        }
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

fn list(args: &clap::ArgMatches) -> Result<()> {
    let repo = open_config_repo(args.value_of("data-directory"))?;
    let users = repo.get_users()?;
    for user in users {
        println!("{}", serde_json::to_string(&user).unwrap());
    }
    Ok(())
}

fn get_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn add(args: &clap::ArgMatches) -> Result<()> {
    let repo = open_config_repo(args.value_of("data-directory"))?;
    let username = get_input("Username: ")?;
    if username.is_empty() {
        return Err(anyhow!("empty username not allowed"));
    }
    let password = rpassword::read_password_from_tty(Some("Password: "))?;
    if password.is_empty() {
        return Err(anyhow!("empty password not allowed"));
    }
    let confirm = rpassword::read_password_from_tty(Some("Confirm password: "))?;
    if password != confirm {
        return Err(anyhow!("passwords to not match"));
    }
    repo.add_user(&username, &password)?;
    println!("User added: username=\"{}\"", username);
    Ok(())
}

fn remove(args: &clap::ArgMatches) -> Result<()> {
    let repo = open_config_repo(args.value_of("data-directory"))?;
    let username = args.value_of("username").unwrap();
    if repo.remove_user(username)? == 0 {
        return Err(anyhow!("user does not exist"));
    }
    println!("User removed: username=\"{}\"", username);
    Ok(())
}

fn password(args: &clap::ArgMatches) -> Result<()> {
    let username = args.value_of("username").unwrap();
    let repo = open_config_repo(args.value_of("data-directory"))?;
    let user = repo.get_user_by_name(username)?;
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
