// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::io::{stdin, stdout, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Arg;
use clap::Command;

use crate::sqlite::configrepo::ConfigRepo;

pub fn users_subcommand() -> clap::Command<'static> {
    clap::Command::new("users")
        .subcommand(Command::new("list").alias("ls").about("List users"))
        .subcommand(
            Command::new("add")
                .about("Add user")
                .arg(
                    Arg::new("username")
                        .long("username")
                        .short('u')
                        .value_name("USERNAME")
                        .help("Username"),
                )
                .arg(
                    Arg::new("password")
                        .long("password")
                        .short('p')
                        .value_name("PASSWORD")
                        .help("Password"),
                ),
        )
        .subcommand(
            Command::new("rm")
                .about("Remove user")
                .arg(Arg::new("username").required(true)),
        )
        .subcommand(
            Command::new("passwd")
                .alias("password")
                .about("Change password for user")
                .arg(Arg::new("username").required(true)),
        )
}

pub fn main(args: &clap::ArgMatches) -> Result<()> {
    match args.subcommand() {
        Some(("list", args)) => list(args),
        Some(("add", args)) => add(args),
        Some(("rm", args)) => remove(args),
        Some(("passwd", args)) => password(args),
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
    print!("{prompt}");
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn add(args: &clap::ArgMatches) -> Result<()> {
    let repo = open_config_repo(args.value_of("data-directory"))?;

    let username = if let Some(username) = args.value_of("username") {
        username.to_string()
    } else {
        get_input("Username: ")?
    };
    if username.is_empty() {
        return Err(anyhow!("empty username not allowed"));
    }

    let password = if let Some(password) = args.value_of("password") {
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

fn remove(args: &clap::ArgMatches) -> Result<()> {
    let repo = open_config_repo(args.value_of("data-directory"))?;
    let username = args.value_of("username").unwrap();
    if repo.remove_user(username)? == 0 {
        return Err(anyhow!("user does not exist"));
    }
    println!("User removed: username=\"{username}\"");
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
