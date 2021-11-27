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

use crate::elastic::request::Request;
use crate::elastic::{self, Client};
use crate::types::JsonValue;
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

    if let Err(err) = check_logstash(&client).await {
        println!("Failed to check logstash-* for Suricata events: {}", err);
    }
    if let Err(err) = check_filebeat(&client).await {
        println!("Failed to check filebeat-* for Suricata events: {}", err);
    }
    if let Err(err) = check_filebeat_ecs(&client).await {
        println!(
            "Failed to check filebeat-* for Suricata ECS events: {}",
            err
        );
    }

    Ok(())
}

async fn check_logstash(client: &Client) -> anyhow::Result<()> {
    let index_pattern = "logstash-*";
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::exists_filter("event_type"));
    request.push_filter(elastic::request::exists_filter("src_ip"));
    request.push_filter(elastic::request::exists_filter("dest_ip"));
    request.size(1);
    let response: JsonValue = client
        .post(&format!("{}/_search", index_pattern))?
        .json(&request)
        .send()
        .await?
        .json()
        .await?;
    let mut found = false;
    if let Some(hits) = response["hits"]["hits"].as_array() {
        if !hits.is_empty() {
            found = true;
        }
    }

    if found {
        println!("Found Suricata events at index pattern {}", index_pattern);
    } else {
        println!(
            "No Suricata events found at index pattern {}",
            index_pattern
        );
    }

    Ok(())
}

async fn check_filebeat(client: &Client) -> anyhow::Result<()> {
    let index_pattern = "filebeat-*";
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::exists_filter("event_type"));
    request.push_filter(elastic::request::exists_filter("src_ip"));
    request.push_filter(elastic::request::exists_filter("dest_ip"));
    request.size(1);
    let response: JsonValue = client
        .post(&format!("{}/_search", index_pattern))?
        .json(&request)
        .send()
        .await?
        .json()
        .await?;
    let mut found = false;
    if let Some(hits) = response["hits"]["hits"].as_array() {
        if !hits.is_empty() {
            found = true;
        }
    }

    if found {
        println!("Found Suricata events at index pattern {}", index_pattern);
    } else {
        println!(
            "No Suricata events found at index pattern {}",
            index_pattern
        );
    }

    Ok(())
}

async fn check_filebeat_ecs(client: &Client) -> anyhow::Result<()> {
    let index_pattern = "filebeat-*";
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::exists_filter("ecs"));
    request.push_filter(elastic::request::exists_filter("suricata.eve.event_type"));
    request.size(1);
    let response: JsonValue = client
        .post(&format!("{}/_search", index_pattern))?
        .json(&request)
        .send()
        .await?
        .json()
        .await?;

    let mut found = false;
    if let Some(hits) = response["hits"]["hits"].as_array() {
        if !hits.is_empty() {
            found = true;
        }
    }

    if found {
        println!(
            "Found Suricata ECS events at index pattern {}",
            index_pattern
        );
    } else {
        println!(
            "No Suricata ECS events found at index pattern {}",
            index_pattern
        );
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct Index {
    pub index: String,
}

#[derive(Debug, Deserialize)]
struct Template {}
