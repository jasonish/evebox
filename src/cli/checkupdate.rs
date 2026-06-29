// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! `evebox check-update` -- check whether a newer EveBox release is available.
//!
//! Fetches a small JSON manifest published alongside the release downloads and
//! compares the latest stable release against the running version. The check is
//! always user initiated (this command, or the button in the web UI); EveBox
//! never reaches out on its own.
//!
//! Development builds (versions with a `-dev` pre-release tag) are compared
//! against the latest *stable* release only. Thanks to SemVer pre-release
//! ordering this does the right thing automatically: `0.26.0-dev` sorts before
//! the `0.26.0` release (so once `0.26.0` ships, a `0.26.0-dev` build is told a
//! release is available) but after `0.25.0` (so a dev build ahead of the latest
//! release is never reported as out of date).

use crate::cli::prelude::*;

use anyhow::Context;
use serde::Deserialize;
use tracing::debug;

/// Default URL of the release manifest. Served from the EveBox download host
/// with permissive CORS so the web UI can fetch it directly as well.
const DEFAULT_MANIFEST_URL: &str = "https://evebox.org/files/release/latest.json";

#[derive(Debug, Parser)]
#[command(
    name = "check-update",
    about = "Check if a new EveBox release is available"
)]
pub struct Args {
    /// Override the manifest URL (for testing or mirrors).
    #[arg(
        long,
        value_name = "URL",
        default_value = DEFAULT_MANIFEST_URL,
        env = "EVEBOX_UPDATE_URL",
        hide_env = true
    )]
    url: String,

    /// Only produce output when an update is available.
    #[arg(long)]
    quiet: bool,
}

/// The release manifest published at the manifest URL.
#[derive(Debug, Deserialize)]
struct Manifest {
    /// Latest stable release version, e.g. "0.26.0".
    version: String,
}

pub fn args() -> Command {
    Args::command()
}

pub async fn main(args: &ArgMatches) -> Result<()> {
    let args = Args::from_arg_matches(args)?;
    run(&args).await
}

async fn run(args: &Args) -> Result<()> {
    let current_str = crate::version::version();
    let current = semver::Version::parse(current_str)
        .with_context(|| format!("Failed to parse running version: {current_str}"))?;

    debug!("Fetching update manifest from {}", args.url);
    let manifest = fetch_manifest(&args.url).await?;

    let latest = semver::Version::parse(&manifest.version).with_context(|| {
        format!(
            "Failed to parse latest version from manifest: {}",
            manifest.version
        )
    })?;

    if current < latest {
        println!("A new EveBox release is available: {latest} (you are running {current_str}).");
    } else if !current.pre.is_empty() {
        // A development build at or ahead of the latest stable release.
        if !args.quiet {
            println!(
                "You are running development build {current_str}; the latest stable release is {latest}."
            );
        }
    } else if !args.quiet {
        println!("EveBox is up to date (running {current_str}, latest is {latest}).");
    }

    Ok(())
}

async fn fetch_manifest(url: &str) -> Result<Manifest> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("EveBox/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch update manifest from {url}"))?
        .error_for_status()
        .with_context(|| format!("Update manifest request failed: {url}"))?;
    response
        .json::<Manifest>()
        .await
        .context("Failed to parse update manifest")
}
