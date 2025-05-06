// SPDX-FileCopyrightText: (C) 2025 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

// Command to convert eve.json packet or payload data to a pcap
// file. Initial version written by Claude, and it worked.

use crate::cli::prelude::*;
use crate::util::pcap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tracing::{error, info, warn};

/// Command line arguments for the eve2pcap command
#[derive(Debug, Parser)]
pub(super) struct Args {
    /// Output PCAP file
    #[arg(short = 'o', long = "output", required = true)]
    output: PathBuf,

    /// Extract payload field instead of packet field
    #[arg(short, long = "payload")]
    payload: bool,

    /// Input EVE JSON file(s) to read from
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
}

/// Run the eve2pcap command
pub(super) async fn main(args: Args) -> Result<()> {
    if args.inputs.len() > 1 {
        info!("Reading EVE data from {} files", args.inputs.len());
    } else {
        info!("Reading EVE data from {}", args.inputs[0].display());
    }

    let event_type = if args.payload { "payload" } else { "packet" };
    info!("Converting {} data to PCAP", event_type);
    info!("Output file: {}", args.output.display());

    let mut output =
        File::create(&args.output).map_err(|e| anyhow!("Failed to create output file: {}", e))?;
    let mut header_done = false;
    for input_path in &args.inputs {
        process_file(input_path, &mut output, &mut header_done, args.payload)?;
    }

    Ok(())
}

fn process_file(
    input_path: &PathBuf,
    output: &mut File,
    header_done: &mut bool,
    payload: bool,
) -> Result<()> {
    info!("Processing file: {}", input_path.display());

    let file = match File::open(input_path) {
        Ok(file) => file,
        Err(err) => {
            error!(
                "Failed to open input file {}: {}",
                input_path.display(),
                err
            );
            // Continue with next file.
            return Ok(());
        }
    };

    let reader = BufReader::new(file);
    for (lineno, result) in reader.lines().enumerate() {
        match result {
            Ok(line) => process_line(&line, lineno, output, header_done, payload),
            Err(err) => {
                warn!("Error reading line: {}", err);
            }
        }
    }

    Ok(())
}

/// Process a single line of EVE JSON
fn process_line(
    line: &str,
    lineno: usize,
    output: &mut File,
    header_one: &mut bool,
    payload: bool,
) {
    // Parse the JSON
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(event) => {
            if (event["payload"].is_string() && payload) || event["packet"].is_string() {
                process_event(&event, lineno, output, header_one, payload);
            }
        }
        Err(err) => {
            warn!("Failed to parse JSON on line {}: {}", lineno + 1, err);
        }
    }
}

fn process_event(
    event: &serde_json::Value,
    lineno: usize,
    output: &mut File,
    header_done: &mut bool,
    payload: bool,
) {
    // Convert EVE to PCAP based on event type
    let result = if payload {
        pcap::payload_to_pcap(event)
    } else {
        pcap::packet_to_pcap(event)
    };

    match result {
        Ok(pcap_data) => {
            write_pcap(&pcap_data, output, header_done).unwrap_or_else(|e| {
                error!("Failed to write PCAP data: {}", e);
            });
        }
        Err(err) => {
            warn!("Failed to convert event on line {}: {}", lineno + 1, err);
        }
    }
}

/// Write PCAP data to the output file, handling headers appropriately
fn write_pcap(pcap_file: &[u8], output_file: &mut File, pcap_header_done: &mut bool) -> Result<()> {
    let buf = if *pcap_header_done {
        // Skip writing the header if already written.
        if pcap_file.len() > pcap::FILE_HEADER_LEN {
            &pcap_file[pcap::FILE_HEADER_LEN..]
        } else {
            pcap_file
        }
    } else {
        // First time, include the entire buffer with header
        *pcap_header_done = true;
        pcap_file
    };

    output_file
        .write_all(buf)
        .map_err(|e| anyhow!("Failed to write to output file: {}", e))
}
