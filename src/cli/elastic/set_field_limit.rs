// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Parser;
use serde_json::json;
use tracing::info;

use crate::elastic::{util::update_template_field_limit, Client};

#[derive(Debug, Clone, Parser)]
pub(crate) struct Args {
    #[clap(flatten)]
    pub elastic: super::main::ElasticOptions,

    /// Field count limit.
    #[clap(long, default_value = "5000")]
    pub limit: usize,

    /// Index prefix
    #[clap(long, default_value = "logstash")]
    pub index: String,
}

pub(crate) async fn main(args: Args) -> Result<()> {
    let mut client = crate::elastic::client::ClientBuilder::new(&args.elastic.elasticsearch)
        .disable_certificate_validation(true);
    if let Some(username) = &args.elastic.username {
        client = client.with_username(username);
    }
    if let Some(password) = &args.elastic.password {
        client = client.with_password(password);
    }
    let client = client.build();

    for index in client
        .get_indices_pattern(&format!("{}*", args.index))
        .await?
    {
        info!("Updating index {}", index.index);
        update_index(&client, &index.index, args.limit).await?;
    }

    info!("Updating template for pattern {}*", args.index);
    update_template_field_limit(&client, &args.index, args.limit).await?;

    Ok(())
}

async fn update_index(client: &Client, index: &str, limit: usize) -> Result<()> {
    #[rustfmt::skip]
    let request = json!({
	"index.mapping.total_fields.limit": limit,
    });

    let response = client
        .put_json(&format!("{}/_settings", index), request)?
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    info!("Index {}: status: {}, body: {}", index, status, body);
    Ok(())
}
