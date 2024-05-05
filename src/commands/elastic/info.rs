// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::elastic::client::InfoResponse;
use crate::elastic::request::Request;
use crate::elastic::{self, Client};

pub async fn main(args: super::main::ElasticOptions) -> anyhow::Result<()> {
    let mut client = Client::new(&args.elasticsearch);

    if args.username.is_some() {
        client.username.clone_from(&args.username);
    }

    if args.password.is_some() {
        client.password.clone_from(&args.password);
    }

    let server_info = get_info(&mut client).await?;
    let ignore_dot = true;
    if let Some(distribution) = &server_info.version.distribution {
        println!("Distribution: {distribution}");
    }
    println!("Version: {}", server_info.version.number);
    if let Some(tagline) = &server_info.tagline {
        println!("Tagline: {tagline}");
    }

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

async fn get_info(client: &mut Client) -> Result<InfoResponse> {
    match client.get_info().await {
        Ok(info) => return Ok(info),
        Err(err) => {
            if client.url.starts_with("https") {
                println!("Failed to get server info, will disable certificate validation and try again: error = {err}");
                client.disable_certificate_validation = true;
            } else {
                anyhow::bail!(err);
            }
        }
    }

    Ok(client.get_info().await?)
}

async fn check_logstash(client: &Client) -> anyhow::Result<()> {
    let index_pattern = "logstash-*";
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::exists_filter("event_type"));
    request.push_filter(elastic::request::exists_filter("src_ip"));
    request.push_filter(elastic::request::exists_filter("dest_ip"));
    request.size(1);
    let response: serde_json::Value = client
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
    let response: serde_json::Value = client
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
    let response: serde_json::Value = client
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
