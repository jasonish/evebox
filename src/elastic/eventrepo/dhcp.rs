// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

use super::ElasticEventRepo;
use crate::{
    elastic::request::{term_filter, timestamp_gte_filter},
    eventrepo::DatastoreError,
    prelude::*,
};
use time::OffsetDateTime;

impl ElasticEventRepo {
    pub async fn dhcp(
        &self,
        earliest: Option<OffsetDateTime>,
        dhcp_type: &str,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        let mut filters = vec![];

        if let Some(earliest) = &earliest {
            filters.push(timestamp_gte_filter(earliest));
        }
        if let Some(sensor) = &sensor {
            filters.push(term_filter(&self.map_field("host"), sensor));
        }

        filters.push(term_filter(&self.map_field("dhcp.dhcp_type"), dhcp_type));

        #[rustfmt::skip]
        let request = json!({
            "query": {
		"bool": {
                    "filter": filters,
		}
            },
            "collapse": {
		"field": self.map_field("dhcp.client_mac"),
            },
	    "sort": [
		{
		    "@timestamp": {
			"order": "desc",
		    },
		}
	    ],
            "size": 10000,
        });

        let response: serde_json::Value = self.search(&request).await?.json().await?;
        let mut events = vec![];

        if let Some(hits) = response["hits"]["hits"].as_array() {
            for hit in hits {
                let mut hit = hit.clone();
                self.transform_ecs(&mut hit);
                let source = &hit["_source"];
                if source.is_object() {
                    events.push(source.clone());
                }
            }
        }

        Ok(events)
    }

    pub async fn dhcp_request(
        &self,
        earliest: Option<OffsetDateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        self.dhcp(earliest, "request", sensor).await
    }

    pub async fn dhcp_ack(
        &self,
        earliest: Option<OffsetDateTime>,
        sensor: Option<String>,
    ) -> Result<Vec<serde_json::Value>, DatastoreError> {
        self.dhcp(earliest, "ack", sensor).await
    }
}
