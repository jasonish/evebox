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

use crate::elastic::client::Client;
use crate::logger::log;
use anyhow::anyhow;
use anyhow::Result;

pub async fn install_template(client: &Client, template: &str) -> Result<()> {
    log::debug!("Checking for template \"{}\"", template);
    match client.get_template(template).await {
        Err(err) => {
            log::warn!("Failed to check if template {} exists: {}", template, err);
        }
        Ok(None) => {
            log::debug!("Did not find template for \"{}\", will install", template);
        }
        Ok(Some(_)) => {
            log::debug!("Found template for \"{}\"", template);
            return Ok(());
        }
    };

    let version = client.get_version().await?;
    if version.major < 7 {
        return Err(anyhow!(
            "Elasticsearch version {} not supported",
            version.version
        ));
    }

    let template_string = {
        if version.major >= 7 {
            crate::resource::get_string("elasticsearch/template-es7x.json").ok_or_else(|| {
                anyhow!(
                    "Failed to find template for Elasticsearch version {}",
                    version.version
                )
            })?
        } else {
            return Err(anyhow!(
                "Elasticsearch version {} not supported",
                version.version
            ));
        }
    };

    log::info!("Installing template {}", &template);
    let mut templatejs: serde_json::Value = serde_json::from_str(&template_string)?;
    templatejs["index_patterns"] = format!("{}-*", template).into();
    client
        .put_template(template, serde_json::to_string(&templatejs)?)
        .await?;

    Ok(())
}
