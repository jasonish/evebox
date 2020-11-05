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

use super::super::eventstore::EventStore;
use crate::elastic::{self, request::Request};
use crate::{
    datastore::{DatastoreError, EventQueryParams},
    types::JsonValue,
};

pub async fn dhcp_report(
    ds: &EventStore,
    what: &str,
    params: &EventQueryParams,
) -> Result<JsonValue, DatastoreError> {
    match what {
        "ack" => dhcp_report_ack(ds, params).await,
        "request" => dhcp_report_request(ds, params).await,
        _ => Err(anyhow::anyhow!("No DHCP report for {}", what).into()),
    }
}

pub async fn dhcp_report_ack(
    ds: &EventStore,
    params: &EventQueryParams,
) -> Result<JsonValue, DatastoreError> {
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::term_filter("event_type", "dhcp"));
    request.push_filter(elastic::request::term_filter("dhcp.dhcp_type", "ack"));

    if let Some(dt) = params.min_timestamp {
        request.push_filter(elastic::request::timestamp_gte_filter(dt));
    }

    let aggs = json!({
        "client_mac": {
          "terms": {
            "field": "dhcp.client_mac.keyword",
            "size": 10000
          },
          "aggs": {
            "latest": {
              "top_hits": {
                "sort": [
                  {
                    "@timestamp": {"order": "desc"}
                  }
                ],
                "size": 1
              }
            }
          }
        }
    });

    request["aggs"] = aggs;
    request.size(0);

    let response: JsonValue = ds.search(&request).await?.json().await?;

    let mut results = Vec::new();

    if let Some(buckets) = response["aggregations"]["client_mac"]["buckets"].as_array() {
        for bucket in buckets {
            let latest = &bucket["latest"]["hits"]["hits"][0]["_source"];
            results.push(latest);
        }
    }

    Ok(json!({
        "data": results,
    }))
}

pub async fn dhcp_report_request(
    ds: &EventStore,
    params: &EventQueryParams,
) -> Result<JsonValue, DatastoreError> {
    let mut request = elastic::request::new_request();
    request.push_filter(elastic::request::term_filter("event_type", "dhcp"));
    request.push_filter(elastic::request::term_filter("dhcp.dhcp_type", "request"));

    if let Some(dt) = params.min_timestamp {
        request.push_filter(elastic::request::timestamp_gte_filter(dt));
    }

    let aggs = json!({
        "client_mac": {
          "terms": {
            "field": "dhcp.client_mac.keyword",
            "size": 10000
          },
          "aggs": {
            "latest": {
              "top_hits": {
                "sort": [
                  {
                    "@timestamp": {
                      "order": "desc"
                    }
                  }
                ],
                "size": 1
              }
            }
          }
        }
    });

    request["aggs"] = aggs;
    request.size(0);

    let response: JsonValue = ds.search(&request).await?.json().await?;

    let mut results = Vec::new();

    if let Some(buckets) = response["aggregations"]["client_mac"]["buckets"].as_array() {
        for bucket in buckets {
            let latest = &bucket["latest"]["hits"]["hits"][0]["_source"];
            results.push(latest);
        }
    }

    Ok(json!({
        "data": results,
    }))
}
