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
