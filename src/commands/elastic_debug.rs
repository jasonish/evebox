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

use crate::elastic::Client;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

pub async fn main(args: &clap::ArgMatches<'static>) -> Result<()> {
    let url = args.value_of("elasticsearch").unwrap();
    let client = Client::new(url);
    let version = client.get_version().await?;
    let ignore_dot = true;
    println!("Elasticsearch version: {}", version.version);

    let indices: Vec<Index> = client
        .get("_cat/indices?format=json")?
        .send()
        .await?
        .json()
        .await?;
    for index in indices {
        if ignore_dot && index.index.starts_with('.') {
            continue;
        }
        println!("Found index: {}", index.index);
    }

    let templates: HashMap<String, Template> =
        client.get("_template")?.send().await?.json().await?;
    for (name, template) in templates {
        if ignore_dot && name.starts_with('.') {
            continue;
        }
        println!("Template: {} => {:?}", name, template);
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct Index {
    pub index: String,
}

#[derive(Debug, Deserialize)]
struct Template {
    pub version: Option<u64>,
    pub index_patterns: Option<Vec<String>>,
}
