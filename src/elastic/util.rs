// SPDX-FileCopyrightText: (C) 2024 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use tracing::{error, info};

use super::Client;

pub(crate) async fn check_and_set_field_limit(client: &Client, template_name: &str) {
    match client.get_template(template_name).await {
        Ok(template) => {
            let field_limit = &template["settings"]["index"]["mapping"]["total_fields"]["limit"];
            let limit: Option<i64> = match field_limit {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse::<i64>().ok(),
                _ => None,
            };
            if let Some(limit) = limit {
                if limit >= 5000 {
                    info!("Field limit of {} OK, will not increase", limit);
                    return;
                }
            }
        }
        Err(err) => {
            info!(
                "Failed to find template for index {}: {}",
                template_name, err
            );
        }
    }

    info!("Attempting to increase Elasticsearch field limit to 5000");
    match update_template_field_limit(client, template_name, 5000).await {
        Ok(_ok) => {
            info!("Successfully updated Elasticsearch template field limit");
        }
        Err(err) => {
            error!(
                "Failed to update Elasticsearch template field limit: {}",
                err
            );
        }
    }
}

pub(crate) async fn update_template_field_limit(
    client: &Client,
    index: &str,
    limit: usize,
) -> anyhow::Result<()> {
    #[rustfmt::skip]
    let request = json!({
	"index_patterns": [
            format!("{}*", index),
	],
	"settings": {
            "index": {
		"mapping": {
		    "total_fields": {
			"limit": limit,
		    }
		}
            }
	}
    });

    let response = client
        .put(&format!("_template/{index}"))?
        .json(&request)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    info!("Template {}: status: {}, body: {}", index, status, body);

    Ok(())
}
