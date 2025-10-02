// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::server::ServerContext;
use crate::server::main::SessionExtractor;
use crate::util::pcap;

use std::sync::Arc;

use axum::extract::{Extension, Form};
use axum::response::IntoResponse;
use serde::Deserialize;

use hyper::header::{CONTENT_DISPOSITION, CONTENT_TYPE, HeaderMap};

use crate::error::AppError;

#[derive(Deserialize, Debug)]
pub(crate) struct PcapForm {
    pub what: String,
    pub event: String,
}

pub(crate) async fn handler(
    Extension(_context): Extension<Arc<ServerContext>>,
    _session: SessionExtractor,
    Form(form): Form<PcapForm>,
) -> Result<impl IntoResponse, AppError> {
    let mut hmap = HeaderMap::new();
    hmap.insert(
        CONTENT_TYPE,
        "application/vnc.tcpdump.pcap".parse().unwrap(),
    );

    let event: serde_json::Value = serde_json::from_str(&form.event)
        .map_err(|err| AppError::BadRequest(format!("failed to decode event: {err}")))?;

    let pcap_buffer = pcap::eve_to_pcap(&form.what, &event)?;
    let filename = generate_filename(&event);

    let cs_hdr_value = format!("attachment; filename={filename}.pcap");
    hmap.insert(CONTENT_DISPOSITION, cs_hdr_value.parse().unwrap());

    Ok((hmap, pcap_buffer))
}

/// Generate a filename for the PCAP file based on the event
fn generate_filename(event: &serde_json::Value) -> String {
    if let Some(sid) = &event["alert"]["signature_id"].as_u64() {
        sid.to_string()
    } else {
        "event".to_string()
    }
}
