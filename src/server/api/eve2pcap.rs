// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::eve::Eve;
use crate::pcap;
use crate::server::api::ApiError;
use crate::server::main::SessionExtractor;
use crate::server::ServerContext;

use std::sync::Arc;

use axum::extract::{Extension, Form};
use axum::response::IntoResponse;
use base64::prelude::*;
use serde::Deserialize;
use tracing::warn;

#[derive(Deserialize, Debug)]
pub(crate) struct PcapForm {
    pub what: String,
    pub event: String,
}

pub(crate) async fn handler(
    Extension(_context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
    Form(form): Form<PcapForm>,
) -> Result<impl IntoResponse, ApiError> {
    let headers = [
        ("content-type", "application/vnc.tcpdump.pcap"),
        ("content-disposition", "attachment; filename=event.pcap"),
    ];

    let event: serde_json::Value = serde_json::from_str(&form.event)
        .map_err(|err| ApiError::BadRequest(format!("failed to decode event: {err}")))?;
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
                .map(|s| BASE64_STANDARD.decode(s))
                .ok_or_else(|| ApiError::BadRequest("no packet in event".to_string()))?
                .map_err(|err| {
                    ApiError::BadRequest(format!("failed to base64 decode packet: {err}"))
                })?;
            let ts = event.datetime().ok_or_else(|| {
                ApiError::BadRequest("bad or missing timestamp field".to_string())
            })?;
            let pcap_buffer = pcap::create(linktype, ts, packet);
            Ok((headers, pcap_buffer))
        }
        "payload" => {
            let ts = event.datetime().ok_or_else(|| {
                ApiError::BadRequest("bad or missing timestamp field".to_string())
            })?;
            let packet = pcap::packet_from_payload(&event).map_err(|err| {
                let msg = format!("Failed to create packet from payload: {err:?}");
                warn!("{}", msg);
                ApiError::BadRequest(msg)
            })?;
            let pcap_buffer = pcap::create(pcap::LinkType::Raw as u32, ts, &packet);
            Ok((headers, pcap_buffer))
        }
        _ => Err(ApiError::BadRequest("invalid value for what".to_string())),
    }
}
