// SPDX-License-Identifier: MIT
//
// Copyright (C) 2022 Jason Ish

use crate::elastic::request::Request;
use crate::elastic::{self, Client};
use crate::types::JsonValue;

pub async fn main(args: &clap::ArgMatches) -> anyhow::Result<()> {
    let url = args.value_of("elasticsearch").unwrap();
    let client = Client::new(url);
    let version = client.get_version().await?;
    let ignore_dot = true;
    println!("Elasticsearch version: {}", version.version);

    let indices = client.get_indices().await?;
    for index in indices.keys() {
        if ignore_dot && index.starts_with('.') {
            continue;
        }
        println!("Found index: {index}");
    }

    if let Err(err) = check_logstash(&client).await {
        println!("Failed to check logstash-* for Suricata events: {err}");
    }
    if let Err(err) = check_filebeat(&client).await {
        println!("Failed to check filebeat-* for Suricata events: {err}");
    }
    if let Err(err) = check_filebeat_ecs(&client).await {
        println!("Failed to check filebeat-* for Suricata ECS events: {err}");
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
        .post(&format!("{index_pattern}/_search"))?
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
        println!("Found Suricata events at index pattern {index_pattern}");
    } else {
        println!("No Suricata events found at index pattern {index_pattern}");
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
        .post(&format!("{index_pattern}/_search"))?
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
        println!("Found Suricata events at index pattern {index_pattern}");
    } else {
        println!("No Suricata events found at index pattern {index_pattern}");
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
        .post(&format!("{index_pattern}/_search"))?
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
        println!("Found Suricata ECS events at index pattern {index_pattern}");
    } else {
        println!("No Suricata ECS events found at index pattern {index_pattern}");
    }

    Ok(())
}
