// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Direct rule downloads for local Suricata installations without
//! `suricata-update`.

use crate::prelude::*;

use flate2::read::GzDecoder;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Cursor, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};
use tar::Archive;

const SOURCE_NAMES: [&str; 3] = ["ET/Open", "PAW Patrules", "The Hunters Ledger Open"];

const PAWPATRULES_URL: &str = "https://rules.pawpatrules.fr/suricata/paw-patrules.tar.gz";
const HUNTERS_LEDGER_URL: &str =
    "https://the-hunters-ledger.com/feeds/suricata/hunters-ledger.rules";
const MAX_ARCHIVE_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;
const CACHE_MAX_AGE: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceFormat {
    TarGz,
    Rules,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuleSource {
    name: &'static str,
    url: String,
    format: SourceFormat,
    filename: Option<&'static str>,
    cache_filename: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct DownloadSummary {
    pub(super) rule_files: usize,
    pub(super) archive_files: usize,
}

pub(super) async fn download_and_merge(
    version: &semver::Version,
    rules_directory: &Path,
    output: &Path,
) -> Result<DownloadSummary> {
    let client = reqwest::Client::builder()
        .user_agent(format!("EveBox/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .context("failed to initialize the direct rule downloader")?;
    let cache_directory = match cache_directory(version).and_then(|path| {
        std::fs::create_dir_all(&path).with_context(|| {
            format!(
                "failed to create direct rule download cache {}",
                path.display()
            )
        })?;
        Ok(path)
    }) {
        Ok(path) => Some(path),
        Err(err) => {
            warn!("Direct rule download cache is unavailable: {err:#}");
            None
        }
    };

    let mut archive_files = 0;
    for source in rule_sources(version) {
        let cache_path = cache_directory
            .as_ref()
            .map(|directory| directory.join(source.cache_filename));
        let contents = download_source(&client, &source, cache_path.as_deref()).await?;

        match source.format {
            SourceFormat::TarGz => {
                archive_files += extract_tar_gz(&contents, rules_directory)
                    .with_context(|| format!("failed to extract {} rules", source.name))?;
            }
            SourceFormat::Rules => {
                let filename = source
                    .filename
                    .context("direct rule source is missing its output filename")?;
                std::fs::write(rules_directory.join(filename), contents)
                    .with_context(|| format!("failed to save downloaded {} rules", source.name))?;
            }
        }
    }

    let rule_files = concatenate_rules(rules_directory, output)?;
    Ok(DownloadSummary {
        rule_files,
        archive_files,
    })
}

fn cache_directory(version: &semver::Version) -> Result<PathBuf> {
    Ok(cache_root(version)?.join("downloads"))
}

fn cache_root(version: &semver::Version) -> Result<PathBuf> {
    let directories = directories::ProjectDirs::from("org", "evebox", "evebox")
        .context("failed to determine a cache directory for local Suricata data")?;
    Ok(directories
        .cache_dir()
        .join("oneshot-suricata")
        .join("local")
        .join(format!(
            "{}.{}.{}",
            version.major, version.minor, version.patch
        )))
}

async fn download_source(
    client: &reqwest::Client,
    source: &RuleSource,
    cache_path: Option<&Path>,
) -> Result<Vec<u8>> {
    let cached = if let Some(cache_path) = cache_path {
        match std::fs::read(cache_path) {
            Ok(contents) if cache_is_fresh(cache_path) => {
                info!(
                    "Using cached {} rules from {}",
                    source.name,
                    cache_path.display()
                );
                return Ok(contents);
            }
            Ok(contents) => Some(contents),
            Err(_) => None,
        }
    } else {
        None
    };

    info!("Downloading {} rules from {}", source.name, source.url);
    let downloaded = async {
        let response = client
            .get(&source.url)
            .send()
            .await
            .with_context(|| format!("failed to download {} from {}", source.name, source.url))?
            .error_for_status()
            .with_context(|| format!("failed to download {} from {}", source.name, source.url))?;
        Ok::<_, anyhow::Error>(
            response
                .bytes()
                .await
                .with_context(|| format!("failed to read downloaded {} rules", source.name))?
                .to_vec(),
        )
    }
    .await;

    match downloaded {
        Ok(contents) => {
            if let Some(cache_path) = cache_path
                && let Err(err) = std::fs::write(cache_path, &contents)
            {
                warn!(
                    "Failed to cache {} rules at {}: {}",
                    source.name,
                    cache_path.display(),
                    err
                );
            }
            Ok(contents)
        }
        Err(err) => {
            if let Some(cached) = cached {
                warn!(
                    "Using stale cached {} rules after download failed: {:#}",
                    source.name, err
                );
                Ok(cached)
            } else {
                Err(err)
            }
        }
    }
}

fn cache_is_fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age <= CACHE_MAX_AGE)
}

fn rule_sources(version: &semver::Version) -> Vec<RuleSource> {
    vec![
        RuleSource {
            name: SOURCE_NAMES[0],
            url: format!(
                "https://rules.emergingthreats.net/open/suricata-\
                 {}.{}.{}/emerging.rules.tar.gz",
                version.major, version.minor, version.patch
            ),
            format: SourceFormat::TarGz,
            filename: None,
            cache_filename: "et-open.tar.gz",
        },
        RuleSource {
            name: SOURCE_NAMES[1],
            url: PAWPATRULES_URL.to_string(),
            format: SourceFormat::TarGz,
            filename: None,
            cache_filename: "pawpatrules.tar.gz",
        },
        RuleSource {
            name: SOURCE_NAMES[2],
            url: HUNTERS_LEDGER_URL.to_string(),
            format: SourceFormat::Rules,
            filename: Some("hunters-ledger.rules"),
            cache_filename: "hunters-ledger.rules",
        },
    ]
}

fn extract_tar_gz(contents: &[u8], output_directory: &Path) -> Result<usize> {
    let decoder = GzDecoder::new(Cursor::new(contents));
    let mut archive = Archive::new(decoder);
    let mut extracted_bytes: u64 = 0;
    let mut extracted_files = 0;

    for entry in archive.entries().context("failed to read tar archive")? {
        let mut entry = entry.context("failed to read tar archive entry")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let archive_path = entry
            .path()
            .context("tar archive contains an invalid path")?;
        let Some(relative_path) = normalized_archive_path(&archive_path)? else {
            continue;
        };
        extracted_bytes = extracted_bytes
            .checked_add(entry.size())
            .context("rule archive expanded size overflowed")?;
        if extracted_bytes > MAX_ARCHIVE_OUTPUT_BYTES {
            anyhow::bail!(
                "rule archive expands beyond the {} MiB safety limit",
                MAX_ARCHIVE_OUTPUT_BYTES / 1024 / 1024
            );
        }

        let destination = output_directory.join(relative_path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create rule support directory {}",
                    parent.display()
                )
            })?;
        }
        let mut output = File::create(&destination).with_context(|| {
            format!("failed to create extracted file {}", destination.display())
        })?;
        std::io::copy(&mut entry, &mut output)
            .with_context(|| format!("failed to extract file {}", destination.display()))?;
        extracted_files += 1;
    }

    Ok(extracted_files)
}

fn normalized_archive_path(path: &Path) -> Result<Option<PathBuf>> {
    let mut components = path.components().peekable();
    if components
        .peek()
        .is_some_and(|component| *component == Component::Normal(OsStr::new("rules")))
    {
        components.next();
    }

    let mut normalized = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("rule archive contains unsafe path {}", path.display());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized))
    }
}

fn concatenate_rules(rules_directory: &Path, output: &Path) -> Result<usize> {
    let mut rule_files = Vec::new();
    collect_rule_files(rules_directory, output, &mut rule_files)?;
    rule_files.sort();
    if rule_files.is_empty() {
        anyhow::bail!("the downloaded rule sources did not contain any .rules files");
    }

    let mut merged = BufWriter::new(
        File::create(output)
            .with_context(|| format!("failed to create merged rule file {}", output.display()))?,
    );
    let mut removed = 0_usize;
    for rule_file in &rule_files {
        let mut input = BufReader::new(
            File::open(rule_file)
                .with_context(|| format!("failed to open rule file {}", rule_file.display()))?,
        );
        let mut line = Vec::new();
        let mut ends_with_newline = true;
        loop {
            line.clear();
            let read = input
                .read_until(b'\n', &mut line)
                .with_context(|| format!("failed to read rule file {}", rule_file.display()))?;
            if read == 0 {
                break;
            }
            if rule_is_unsupported(&line) {
                removed += 1;
                continue;
            }
            merged
                .write_all(&line)
                .with_context(|| format!("failed to append rule file {}", rule_file.display()))?;
            ends_with_newline = line.ends_with(b"\n");
        }
        if !ends_with_newline {
            merged.write_all(b"\n")?;
        }
    }
    merged
        .flush()
        .with_context(|| format!("failed to finish merged rule file {}", output.display()))?;
    if removed > 0 {
        info!(
            "Removed {removed} rules using the file.magic or filemagic keywords, \
             which are not supported by Suricata on Windows"
        );
    }
    Ok(rule_files.len())
}

/// The Windows Suricata builds do not include libmagic, so rules using the
/// file.magic or filemagic keywords fail to load there. Commented rules are
/// kept as they are inert.
fn rule_is_unsupported(line: &[u8]) -> bool {
    if !cfg!(windows) {
        return false;
    }
    if line.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'#') {
        return false;
    }
    [b"file.magic".as_slice(), b"filemagic".as_slice()]
        .iter()
        .any(|keyword| line.windows(keyword.len()).any(|window| window == *keyword))
}

fn collect_rule_files(
    directory: &Path,
    output: &Path,
    rule_files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("failed to read rule directory {}", directory.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to read an entry in rule directory {}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect extracted rule path {}", path.display()))?;
        if file_type.is_dir() {
            collect_rule_files(&path, output, rule_files)?;
        } else if file_type.is_file()
            && path != output
            && path.extension() == Some(OsStr::new("rules"))
        {
            rule_files.push(path);
        }
    }
    Ok(())
}
