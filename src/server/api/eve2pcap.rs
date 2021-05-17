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

use std::sync::Arc;

use crate::prelude::*;
use serde::Deserialize;

use crate::eve::eve::EveJson;
use crate::eve::Eve;
use crate::pcap;
use crate::server::api::ApiError;
use crate::server::session::Session;
use crate::server::ServerContext;

#[derive(Deserialize, Debug)]
pub struct Form {
    pub what: String,
    pub event: String,
}

pub async fn handler(
    _context: Arc<ServerContext>,
    _session: Arc<Session>,
    form: Form,
) -> Result<impl warp::Reply, warp::Rejection> {
    let event: EveJson = serde_json::from_str(&form.event)
        .map_err(|err| ApiError::BadRequest(format!("failed to decode event: {}", err)))?;
    match form.what.as_ref() {
        "packet" => {
            let linktype = if let Some(linktype) = &event["xpacket_info"]["linktype"].as_u64() {
                *linktype as u32
            } else {
                warn!("No usable link-type in event, will use ethernet");
                pcap::LinkType::Ethernet as u32
            };

            let packet = &event["packet"]
                .as_str()
                .map(base64::decode)
                .ok_or_else(|| ApiError::BadRequest("no packet in event".to_string()))?
                .map_err(|err| {
                    ApiError::BadRequest(format!("failed to base64 decode packet: {}", err))
                })?;
            let ts = event.timestamp().ok_or_else(|| {
                ApiError::BadRequest("bad or missing timestamp field".to_string())
            })?;
            let pcap_buffer = pcap::create(linktype, ts, packet);
            let response = warp::reply::with_header(
                warp::reply::with_header(
                    pcap_buffer,
                    "content-type",
                    "application/vnc.tcpdump.pcap",
                ),
                "content-disposition",
                "attachment; filename=event.pcap",
            );
            return Ok(response);
        }
        "payload" => {
            let ts = event.timestamp().ok_or_else(|| {
                ApiError::BadRequest("bad or missing timestamp field".to_string())
            })?;
            let packet = pcap::packet_from_payload(&event).map_err(|err| {
                let msg = format!("Failed to create packet from payload: {}", err);
                warn!("{}", msg);
                ApiError::BadRequest(msg)
            })?;
            let pcap_buffer = pcap::create(pcap::LinkType::Raw as u32, ts, &packet);
            let response = warp::reply::with_header(
                warp::reply::with_header(
                    pcap_buffer,
                    "content-type",
                    "application/vnc.tcpdump.pcap",
                ),
                "content-disposition",
                "attachment; filename=event.pcap",
            );
            return Ok(response);
        }
        _ => {
            return Err(ApiError::BadRequest("invalid value for what".to_string()).into());
        }
    }
}
