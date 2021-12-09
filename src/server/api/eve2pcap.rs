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

use axum::extract::{Extension, Form};
use axum::http::header::HeaderName;
use axum::http::HeaderValue;
use axum::response::{Headers, IntoResponse};
use std::sync::Arc;

use crate::prelude::*;
use serde::Deserialize;

use crate::eve::eve::EveJson;
use crate::eve::Eve;
use crate::pcap;
use crate::server::api::ApiError;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;

#[derive(Deserialize, Debug)]
pub struct PcapForm {
    pub what: String,
    pub event: String,
}

pub(crate) async fn handler(
    Extension(_context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
    Form(form): Form<PcapForm>,
) -> Result<impl IntoResponse, ApiError> {
    let headers = Headers(vec![
        (
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/vnc.tcpdump.pcap"),
        ),
        (
            HeaderName::from_static("content-disposition"),
            HeaderValue::from_static("attachment; filename=event.pcap"),
        ),
    ]);

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
            return Ok((headers, pcap_buffer));
        }
        "payload" => {
            let ts = event.timestamp().ok_or_else(|| {
                ApiError::BadRequest("bad or missing timestamp field".to_string())
            })?;
            let packet = pcap::packet_from_payload(&event).map_err(|err| {
                let msg = format!("Failed to create packet from payload: {:?}", err);
                warn!("{}", msg);
                ApiError::BadRequest(msg)
            })?;
            let pcap_buffer = pcap::create(pcap::LinkType::Raw as u32, ts, &packet);
            return Ok((headers, pcap_buffer));
        }
        _ => {
            return Err(ApiError::BadRequest("invalid value for what".to_string()));
        }
    }
}
