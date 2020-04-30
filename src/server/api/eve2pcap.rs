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

use std::sync::Arc;

use serde::Deserialize;

use crate::eve::eve::EveJson;
use crate::eve::Eve;
use crate::logger::log;
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
            let pcap_buffer = pcap::create(pcap::LinkType::Ethernet, ts, packet);
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
                log::warn!("{}", msg);
                ApiError::BadRequest(msg)
            })?;
            let pcap_buffer = pcap::create(pcap::LinkType::Raw, ts, &packet);
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
