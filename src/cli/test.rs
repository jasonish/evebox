// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! `evebox test elastic` — Elasticsearch/OpenSearch backend test.
//!
//! Loads a bounded sample of real EVE events into a throwaway index and then
//! exercises the *actual* [`ElasticEventRepo`] query and mutation code paths
//! EveBox uses during normal operation, reporting pass/fail per operation. The
//! point is to verify that EveBox works against a given Elasticsearch or
//! OpenSearch version, not to validate event contents — an empty-but-accepted
//! query result counts as a pass.
//!
//! Elasticsearch and OpenSearch speak the same wire protocol here, so
//! `evebox test opensearch` is an alias for `evebox test elastic`.
//!
//! The companion harness `testing/backends/run.sh` is the one-shot
//! integration test: it runs `evebox test sqlite` and then this command
//! against a matrix of container versions.
//!
//! `evebox test sqlite` runs equivalent behavioral checks against a throwaway
//! SQLite database, including the `is:archived` / `is:escalated` alert search
//! filters exercised over both SQLite alert code paths.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::datetime::DateTime;
use crate::elastic::{self, ClientBuilder, ElasticEventRepo, TAG_ARCHIVED, TAG_AUTO_ARCHIVED};
use crate::eve::Eve;
use crate::eve::filters::EveFilterChain;
use crate::eve::reader::EveReader;
use crate::eventrepo::{AggAlert, AlertsResult, EventQueryParams, EventRepo, StatsAggQueryParams};
use crate::importer::EventSink;
use crate::queryparser;
use crate::server::api::AlertGroupSpec;
use crate::server::autoarchive::AutoArchive;
use crate::server::metrics::Metrics;
use crate::server::session::Session;
use crate::sqlite::ConnectionBuilder;
use crate::sqlite::configdb::{EventFilter, FilterEntry};
use crate::sqlite::connection::{init_event_db, open_pool};
use crate::sqlite::eventrepo::SqliteEventRepo;

#[derive(Parser, Debug)]
#[command(name = "test", about = "Test supported backends")]
pub(crate) struct TestArgs {
    #[command(subcommand)]
    command: TestCommand,
}

#[derive(Subcommand, Debug)]
enum TestCommand {
    /// Test the Elasticsearch/OpenSearch backend against a corpus of EVE events
    #[command(visible_alias = "opensearch")]
    Elastic(Args),

    /// Test the SQLite backend against a corpus of EVE events
    Sqlite(SqliteArgs),
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Elasticsearch/OpenSearch URL
    #[clap(
        short,
        long,
        default_value = "http://localhost:9200",
        env = "EVEBOX_ELASTICSEARCH_URL",
        hide_env = true
    )]
    elasticsearch: String,

    /// Username
    #[clap(short, long, env = "EVEBOX_ELASTICSEARCH_USERNAME", hide_env = true)]
    username: Option<String>,

    /// Password
    #[clap(short, long, env = "EVEBOX_ELASTICSEARCH_PASSWORD", hide_env = true)]
    password: Option<String>,

    /// CA certificate filename
    #[clap(long, env = "EVEBOX_ELASTICSEARCH_CACERT", hide_env = true)]
    cacert: Option<String>,

    /// Disable TLS certificate validation
    #[clap(short = 'k', long)]
    no_check_certificate: bool,

    /// Index prefix. In import mode a unique per-run suffix is appended and the
    /// index is created and deleted by the test (default: evebox-backend-test).
    /// With --existing this selects the existing index prefix to query
    /// (default: logstash).
    #[clap(long)]
    index: Option<String>,

    /// Test against an existing datastore without importing (read-only).
    ///
    /// Runs only read queries — performs no imports, mutations, or deletions —
    /// so it is safe to run against a production cluster. Select the index
    /// prefix to query with --index (default: logstash).
    #[clap(long, conflicts_with = "inputs")]
    existing: bool,

    /// Maximum number of events to import
    #[clap(long, default_value_t = 20000)]
    limit: usize,

    /// Keep the test index after the run instead of deleting it (import mode
    /// only)
    #[clap(long)]
    keep: bool,

    /// Emit results as JSON
    #[clap(long)]
    json: bool,

    /// EVE files or directories to sample events from (directories are
    /// searched recursively for *.json files). Required unless --existing.
    #[clap(value_name = "INPUT", required_unless_present = "existing")]
    inputs: Vec<PathBuf>,
}

#[derive(Parser, Debug)]
pub(crate) struct SqliteArgs {
    /// Maximum number of events to import
    #[clap(long, default_value_t = 20000)]
    limit: usize,

    /// Database file to use instead of a throwaway temporary database. When
    /// given, it is created if needed and kept after the run.
    #[clap(long)]
    database: Option<PathBuf>,

    /// Emit results as JSON
    #[clap(long)]
    json: bool,

    /// EVE files or directories to sample events from (directories are searched
    /// recursively for *.json files).
    #[clap(value_name = "INPUT", required = true)]
    inputs: Vec<PathBuf>,
}

/// Default index prefix for import mode (a unique per-run suffix is appended).
const DEFAULT_IMPORT_INDEX: &str = "evebox-backend-test";

/// Default existing-index prefix for --existing mode (EveBox's own default).
const DEFAULT_EXISTING_INDEX: &str = "logstash";

pub fn command() -> Command {
    TestArgs::command()
}

pub async fn main(args: &clap::ArgMatches) -> Result<()> {
    let args = TestArgs::from_arg_matches(args)?;
    match args.command {
        TestCommand::Elastic(args) => run_elastic(&args).await,
        TestCommand::Sqlite(args) => run_sqlite(&args).await,
    }
}

async fn run_elastic(args: &Args) -> Result<()> {
    let report = run(args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_human();
    }
    if report.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pass,
    Fail,
    /// A failure that matches a documented EveBox limitation rather than an
    /// engine incompatibility. Does not count as a failure.
    Known,
    Skip,
}

/// Error substring identifying the known free-text-search limitation: a
/// field-less `query_string` expands across every field and exceeds the
/// engine's field-expansion limit on indices with very wide mappings (Suricata
/// EVE mappings can exceed 1024 fields). This is an EveBox limitation, not a
/// version incompatibility — it is the same on Elasticsearch and OpenSearch,
/// and does not occur on engines whose field-expansion limit is high enough
/// (e.g. Elasticsearch 8.x) or on narrower mappings.
const FIELD_EXPANSION_LIMITATION: &str = "field expansion for [*] matches too many fields";

#[derive(Serialize)]
struct Check {
    name: String,
    status: Status,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl Check {
    fn finish(name: &str, start: Instant, result: Result<Option<String>>) -> Check {
        let duration_ms = start.elapsed().as_millis();
        match result {
            Ok(detail) => Check {
                name: name.to_string(),
                status: Status::Pass,
                duration_ms,
                detail,
            },
            Err(err) => Check {
                name: name.to_string(),
                status: Status::Fail,
                duration_ms,
                detail: Some(err.to_string()),
            },
        }
    }

    fn pass(name: &str, start: Instant, detail: Option<String>) -> Check {
        Check::finish(name, start, Ok(detail))
    }

    fn fail(name: &str, start: Instant, message: String) -> Check {
        Check::finish(name, start, Err(anyhow!(message)))
    }

    fn skip(name: &str, reason: &str) -> Check {
        Check {
            name: name.to_string(),
            status: Status::Skip,
            duration_ms: 0,
            detail: Some(reason.to_string()),
        }
    }

    /// Like [`Check::finish`], but a failure whose error contains any of
    /// `known_markers` is recorded as [`Status::Known`] (a documented
    /// limitation) rather than [`Status::Fail`].
    fn finish_lenient(
        name: &str,
        start: Instant,
        result: Result<Option<String>>,
        known_markers: &[&str],
    ) -> Check {
        match result {
            Ok(_) => Check::finish(name, start, result),
            Err(err) => {
                let message = err.to_string();
                if known_markers.iter().any(|m| message.contains(m)) {
                    Check {
                        name: name.to_string(),
                        status: Status::Known,
                        duration_ms: start.elapsed().as_millis(),
                        detail: Some(format!("known limitation: {message}")),
                    }
                } else {
                    Check::finish(name, start, Err(anyhow!(message)))
                }
            }
        }
    }
}

#[derive(Serialize)]
struct Report {
    distribution: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tagline: Option<String>,
    /// Whether the detected version meets EveBox's supported floor.
    supported: bool,
    /// "import" (imported isolated test data) or "existing" (read-only).
    mode: &'static str,
    index: String,
    imported: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
    checks: Vec<Check>,
    passed: usize,
    failed: usize,
    known: usize,
    skipped: usize,
}

impl Report {
    fn print_human(&self) {
        println!("EveBox backend test");
        println!("  Server:   {} {}", self.distribution, self.version);
        if let Some(tagline) = &self.tagline {
            println!("  Tagline:  {tagline}");
        }
        if !self.supported {
            println!("  WARNING:  this version is below EveBox's supported floor");
        }
        for warning in &self.warnings {
            println!("  WARNING:  {warning}");
        }
        println!("  Mode:     {}", self.mode);
        println!("  Index:    {}", self.index);
        println!("  Imported: {} events", self.imported);
        println!();

        let width = self
            .checks
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or(0)
            .max(4);
        for check in &self.checks {
            let label = match check.status {
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
                Status::Known => "KNWN",
                Status::Skip => "SKIP",
            };
            let detail = check.detail.as_deref().unwrap_or("");
            println!(
                "  {label}  {:<width$}  {:>5}ms  {detail}",
                check.name, check.duration_ms
            );
        }
        println!();
        println!(
            "  {} passed, {} failed, {} known-limitations, {} skipped",
            self.passed, self.failed, self.known, self.skipped
        );
    }
}

/// Whether the detected version meets EveBox's supported floor (ES >= 7.10,
/// OpenSearch >= 2.6.0). Unparseable versions are treated as supported.
fn is_supported(distribution: &str, version: &str) -> bool {
    match elastic::Version::parse(version) {
        Ok(v) => {
            if distribution == "opensearch" {
                v.major > 2 || (v.major == 2 && v.minor >= 6)
            } else {
                v.major > 7 || (v.major == 7 && v.minor >= 10)
            }
        }
        Err(_) => true,
    }
}

/// Records a timed check. The body is an async block evaluating to
/// `Result<Option<String>>`: `Ok(detail)` is a pass, `Err` is a failure.
macro_rules! check {
    ($checks:expr, $name:expr, $body:block) => {{
        let __start = std::time::Instant::now();
        let __result: anyhow::Result<Option<String>> = (async $body).await;
        $checks.push(Check::finish($name, __start, __result));
    }};
}

async fn run(args: &Args) -> Result<Report> {
    // Build the client.
    let mut builder = ClientBuilder::new(&args.elasticsearch);
    if let Some(username) = &args.username {
        builder = builder.with_username(username);
    }
    if let Some(password) = &args.password {
        builder = builder.with_password(password);
    }
    if let Some(cacert) = &args.cacert {
        builder = builder.with_cacert(cacert)?;
    }
    builder = builder.disable_certificate_validation(args.no_check_certificate);
    let client = builder.build();

    // Mode + index base.
    //
    // Import mode (default) creates and later deletes its own index, so it uses
    // a unique per-run prefix (a lowercased ULID suffix). This guarantees the
    // `{base}-*` query pattern, every mutation, and the `{base}*` cleanup only
    // ever touch indices created by this run — even if --index is pointed at a
    // real prefix such as `logstash`.
    //
    // Existing mode (--existing) imports nothing and only runs read queries, so
    // it uses the requested prefix verbatim and never writes, mutates, or
    // deletes anything.
    let base = if args.existing {
        args.index
            .clone()
            .unwrap_or_else(|| DEFAULT_EXISTING_INDEX.to_string())
    } else {
        let prefix = args.index.as_deref().unwrap_or(DEFAULT_IMPORT_INDEX);
        format!("{prefix}-{}", ulid::Ulid::new().to_string().to_lowercase())
    };
    let base = base.as_str();

    // Connect / detect server. A failure here is fatal: nothing else can run.
    let info_start = Instant::now();
    let info = client
        .get_info()
        .await
        .map_err(|err| anyhow!("failed to connect to {}: {}", args.elasticsearch, err))?;
    let distribution = info
        .version
        .distribution
        .clone()
        .unwrap_or_else(|| "elasticsearch".to_string());
    let version = info.version.number.clone();
    let supported = is_supported(&distribution, &version);
    let warnings: Vec<String> = elastic::compatibility_warning(&distribution, &version)
        .into_iter()
        .collect();
    info!("Connected to {} {}", distribution, version);
    if !supported {
        warn!(
            "{} {} is below EveBox's supported floor; testing anyway",
            distribution, version
        );
    }
    for warning in &warnings {
        warn!("{warning}");
    }

    let mut checks: Vec<Check> = Vec::new();
    checks.push(Check::pass(
        "info",
        info_start,
        Some(format!("{distribution} {version}")),
    ));

    // Field-limit template (GET + PUT _template). Required before importing so
    // the dns-heavy sample doesn't hit the default field limit. Skipped in
    // --existing mode: it mutates cluster state (a template) and is not needed
    // when we are only reading.
    if args.existing {
        checks.push(Check::skip(
            "field_limit_template",
            "read-only mode (existing data)",
        ));
    } else {
        check!(checks, "field_limit_template", {
            elastic::util::check_and_set_field_limit(&client, base).await;
            let template = client.get_template(base).await?;
            let limit = &template["settings"]["index"]["mapping"]["total_fields"]["limit"];
            Ok(Some(format!("limit={limit}")))
        });
    }

    // Build the repository over the test index.
    let index_pattern = format!("{base}-*");
    let repo = ElasticEventRepo::new(
        base.to_string(),
        index_pattern,
        client.clone(),
        false,
        distribution == "opensearch",
    );

    // Import a sample of events (import mode only).
    let imported = if args.existing {
        checks.push(Check::skip("import", "read-only mode (existing data)"));
        0
    } else {
        let files = collect_inputs(&args.inputs)?;
        if files.is_empty() {
            return Err(anyhow!("no input files found in {:?}", args.inputs));
        }
        info!(
            "Importing up to {} events from {} files",
            args.limit,
            files.len()
        );
        let import_start = Instant::now();
        match repo.get_importer() {
            None => {
                checks.push(Check::fail(
                    "import",
                    import_start,
                    "event importer unavailable (ECS mode is not supported)".to_string(),
                ));
                0
            }
            Some(sink) => match import_events(EventSink::Elastic(sink), &files, args.limit).await {
                Ok((count, types)) => {
                    let summary = types
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    checks.push(Check::pass(
                        "import",
                        import_start,
                        Some(format!("events={count} [{summary}]")),
                    ));
                    count
                }
                Err(err) => {
                    checks.push(Check::fail("import", import_start, err.to_string()));
                    0
                }
            },
        }
    };

    // Make imported events searchable immediately.
    if imported > 0
        && let Err(err) = refresh(&client, base).await
    {
        warn!("Failed to refresh index: {}", err);
    }

    // Index stats (GET _stats).
    check!(checks, "index_stats", {
        let stats = client.get_index_stats(base).await?;
        let docs: u64 = stats.iter().map(|s| s.doc_count).sum();
        Ok(Some(format!("indices={} docs={docs}", stats.len())))
    });

    // Run the query checks against existing data (--existing) or against what we
    // just imported. `mutate` gates the data-modifying checks: in --existing mode
    // they are skipped so we never touch real data.
    if args.existing || imported > 0 {
        let datastore = EventRepo::Elastic(repo.clone());
        let samples = run_common_read_query_checks(&datastore, &mut checks).await;
        run_common_mutation_checks(&datastore, &mut checks, !args.existing, samples).await;
    } else {
        for name in COMMON_QUERY_CHECK_NAMES {
            checks.push(Check::skip(name, "no events imported"));
        }
    }

    if args.existing {
        checks.push(Check::skip(
            "archive_group_exact_fields",
            "read-only mode (existing data)",
        ));
        checks.push(Check::skip(
            "external_auto_archive",
            "read-only mode (existing data)",
        ));
    } else {
        check!(checks, "archive_group_exact_fields", {
            check_elastic_alert_group_exact_fields(&client, &repo, base).await
        });
        check!(checks, "external_auto_archive", {
            check_external_auto_archive(&client, &repo, base).await
        });
    }

    // Cleanup. Never in --existing mode (we created nothing); the unique per-run
    // prefix means import-mode cleanup can only delete this run's indices.
    if args.existing {
        checks.push(Check::skip("cleanup", "read-only mode (existing data)"));
    } else if args.keep {
        info!("Keeping test index {base}* (--keep)");
    } else {
        let start = Instant::now();
        match cleanup(&client, base).await {
            Ok(n) => checks.push(Check::pass("cleanup", start, Some(format!("deleted={n}")))),
            Err(err) => checks.push(Check::fail("cleanup", start, err.to_string())),
        }
    }

    let passed = checks.iter().filter(|c| c.status == Status::Pass).count();
    let failed = checks.iter().filter(|c| c.status == Status::Fail).count();
    let known = checks.iter().filter(|c| c.status == Status::Known).count();
    let skipped = checks.iter().filter(|c| c.status == Status::Skip).count();

    Ok(Report {
        distribution,
        version,
        tagline: info.tagline.clone(),
        supported,
        mode: if args.existing { "existing" } else { "import" },
        index: base.to_string(),
        imported,
        warnings,
        checks,
        passed,
        failed,
        known,
        skipped,
    })
}

/// Insert an alert directly into Elasticsearch/OpenSearch, bypassing EveBox's
/// ingest filters, then exercise the alert-query auto-archive path used when
/// events arrive through Logstash, Filebeat, or another external indexer.
async fn check_external_auto_archive(
    client: &elastic::Client,
    repo: &ElasticEventRepo,
    base: &str,
) -> Result<Option<String>> {
    const SIGNATURE_ID: i64 = 9_999_999;
    const SENSOR: &str = "evebox-backend-test";
    const SRC_IP: &str = "192.0.2.10";
    const DEST_IP: &str = "198.51.100.20";
    const RRNAME: &str = "archive.example";

    let id = ulid::Ulid::new().to_string();
    let nonmatching_id = ulid::Ulid::new().to_string();
    let index = format!("{base}-external-auto-archive");
    let now = DateTime::now();
    let event = serde_json::json!({
        "timestamp": now.to_eve(),
        "@timestamp": now.to_elastic(),
        "event_type": "alert",
        "host": SENSOR,
        "src_ip": SRC_IP,
        "dest_ip": DEST_IP,
        "alert": {
            "signature_id": SIGNATURE_ID,
            "signature": "EveBox external auto-archive integration test",
        },
        "dns": {
            "queries": [{"rrname": RRNAME, "rrtype": "A"}],
        },
    });
    client
        .put(&format!("{index}/_doc/{id}?refresh=true"))?
        .json(&event)
        .send()
        .await?
        .error_for_status()?;

    let older = now - Duration::from_secs(1);
    let nonmatching_event = serde_json::json!({
        "timestamp": older.to_eve(),
        "@timestamp": older.to_elastic(),
        "event_type": "alert",
        "host": SENSOR,
        "src_ip": SRC_IP,
        "dest_ip": DEST_IP,
        "alert": {
            "signature_id": SIGNATURE_ID,
            "signature": "EveBox external auto-archive integration test",
        },
        "dns": {
            "queries": [{"rrname": "keep.example", "rrtype": "A"}],
        },
    });
    client
        .put(&format!("{index}/_doc/{nonmatching_id}?refresh=true"))?
        .json(&nonmatching_event)
        .send()
        .await?
        .error_for_status()?;

    let entry = FilterEntry {
        sensor: Some(SENSOR.to_string()),
        src_ip: Some(SRC_IP.to_string()),
        dest_ip: Some(DEST_IP.to_string()),
        dns_rrname: Some(RRNAME.to_string()),
        tls_sni: None,
        signature_id: SIGNATURE_ID,
        comment: None,
    };
    let mut auto_archive = AutoArchive::default();
    auto_archive.add(&EventFilter::from(&entry));

    let mut repo = repo.clone();
    repo.start_archive_processor();
    repo.alerts(
        elastic::AlertQueryOptions {
            query_string: Some(format!("@sid:{SIGNATURE_ID}")),
            ..Default::default()
        },
        Arc::new(RwLock::new(auto_archive)),
    )
    .await?;

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let document = repo
            .get_event_by_id(id.clone())
            .await?
            .ok_or_else(|| anyhow!("externally indexed test alert disappeared"))?;
        let tags = document["_source"]["tags"].as_array();
        let archived = tags
            .map(|tags| tags.iter().any(|tag| tag == TAG_ARCHIVED))
            .unwrap_or(false);
        let auto_archived = tags
            .map(|tags| tags.iter().any(|tag| tag == TAG_AUTO_ARCHIVED))
            .unwrap_or(false);
        let history = document["_source"]["evebox"]["history"].as_array();
        let filter_query_history = history
            .map(|history| {
                history.iter().any(|entry| {
                    entry["action"] == "auto-archived" && entry["cause"] == "filter-query"
                })
            })
            .unwrap_or(false);
        if archived && auto_archived && filter_query_history {
            let nonmatching = repo
                .get_event_by_id(nonmatching_id.clone())
                .await?
                .ok_or_else(|| anyhow!("nonmatching test alert disappeared"))?;
            let nonmatching_tags = nonmatching["_source"]["tags"].as_array();
            if nonmatching_tags
                .map(|tags| tags.iter().any(|tag| tag == TAG_AUTO_ARCHIVED))
                .unwrap_or(false)
            {
                bail!("nonmatching DNS alert was auto-archived");
            }
            return Ok(Some(format!("document={id}")));
        }
        if Instant::now() >= deadline {
            bail!(
                "externally indexed alert was not auto-archived by query; tags={tags:?}, history={history:?}"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn check_elastic_alert_group_exact_fields(
    client: &elastic::Client,
    repo: &ElasticEventRepo,
    base: &str,
) -> Result<Option<String>> {
    const SENSOR: &str = "evebox-backend-test";
    const SRC_IP: &str = "192.0.2.30";
    const DEST_IP: &str = "198.51.100.40";
    const RRNAME: &str = "discord.com";
    const SNI: &str = "discord.com";
    const DNS_SIGNATURE_ID: u64 = 9_999_997;
    const TLS_SIGNATURE_ID: u64 = 9_999_996;

    let now = DateTime::now();
    let index = format!("{base}-archive-group-exact-fields");
    let dns_match_id = ulid::Ulid::new().to_string();
    let dns_keep_id = ulid::Ulid::new().to_string();
    let tls_match_id = ulid::Ulid::new().to_string();
    let tls_keep_id = ulid::Ulid::new().to_string();

    let events = [
        (
            &dns_match_id,
            json!({
                "timestamp": now.to_eve(),
                "@timestamp": now.to_elastic(),
                "event_type": "alert",
                "host": SENSOR,
                "src_ip": SRC_IP,
                "dest_ip": DEST_IP,
                "alert": {"signature_id": DNS_SIGNATURE_ID, "signature": "Exact DNS archive test"},
                "dns": {"queries": [
                    {"rrname": "keep.example", "rrtype": "A"},
                    {"rrname": RRNAME, "rrtype": "A"},
                ]},
            }),
        ),
        (
            &dns_keep_id,
            json!({
                "timestamp": now.to_eve(),
                "@timestamp": now.to_elastic(),
                "event_type": "alert",
                "host": SENSOR,
                "src_ip": SRC_IP,
                "dest_ip": DEST_IP,
                "alert": {"signature_id": DNS_SIGNATURE_ID, "signature": "Exact DNS archive test"},
                "dns": {"queries": [{"rrname": "keep.example", "rrtype": "A"}]},
            }),
        ),
        (
            &tls_match_id,
            json!({
                "timestamp": now.to_eve(),
                "@timestamp": now.to_elastic(),
                "event_type": "alert",
                "host": SENSOR,
                "src_ip": SRC_IP,
                "dest_ip": DEST_IP,
                "alert": {"signature_id": TLS_SIGNATURE_ID, "signature": "Exact TLS archive test"},
                "tls": {"sni": SNI},
            }),
        ),
        (
            &tls_keep_id,
            json!({
                "timestamp": now.to_eve(),
                "@timestamp": now.to_elastic(),
                "event_type": "alert",
                "host": SENSOR,
                "src_ip": SRC_IP,
                "dest_ip": DEST_IP,
                "alert": {"signature_id": TLS_SIGNATURE_ID, "signature": "Exact TLS archive test"},
                "tls": {"sni": "keep.example"},
            }),
        ),
    ];

    for (id, event) in events {
        client
            .put(&format!("{index}/_doc/{id}?refresh=true"))?
            .json(&event)
            .send()
            .await?
            .error_for_status()?;
    }

    let min_timestamp = (now.clone() - Duration::from_secs(1)).to_rfc3339_utc();
    let max_timestamp = now.to_rfc3339_utc();
    let updated = repo
        .archive_by_alert_group(AlertGroupSpec {
            signature_id: DNS_SIGNATURE_ID,
            src_ip: Some(SRC_IP.to_string()),
            dest_ip: Some(DEST_IP.to_string()),
            sensor: Some(SENSOR.to_string()),
            dns_rrname: Some(RRNAME.to_string()),
            tls_sni: None,
            min_timestamp: min_timestamp.clone(),
            max_timestamp: max_timestamp.clone(),
        })
        .await?;
    if updated != 1 {
        bail!("DNS-constrained alert-group archive updated {updated} events instead of 1");
    }

    let updated = repo
        .archive_by_alert_group(AlertGroupSpec {
            signature_id: TLS_SIGNATURE_ID,
            src_ip: Some(SRC_IP.to_string()),
            dest_ip: Some(DEST_IP.to_string()),
            sensor: Some(SENSOR.to_string()),
            dns_rrname: None,
            tls_sni: Some(SNI.to_string()),
            min_timestamp,
            max_timestamp,
        })
        .await?;
    if updated != 1 {
        bail!("SNI-constrained alert-group archive updated {updated} events instead of 1");
    }

    for (id, expected) in [
        (&dns_match_id, true),
        (&dns_keep_id, false),
        (&tls_match_id, true),
        (&tls_keep_id, false),
    ] {
        let event = repo
            .get_event_by_id(id.to_string())
            .await?
            .ok_or_else(|| anyhow!("exact alert-group test event {id} disappeared"))?;
        let archived = event["_source"]["tags"]
            .as_array()
            .is_some_and(|tags| tags.iter().any(|tag| tag == TAG_ARCHIVED));
        if archived != expected {
            bail!("exact alert-group test event {id} archived={archived}, expected={expected}");
        }
    }

    Ok(Some("dns=1 tls=1".to_string()))
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

async fn run_sqlite(args: &SqliteArgs) -> Result<()> {
    let report = run_sqlite_report(args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_human();
    }
    if report.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Build a SQLite event repository backed by `db_path`, creating the schema.
/// Returns the repository and the reported SQLite library version.
async fn build_sqlite_repo(db_path: &Path) -> Result<(SqliteEventRepo, String)> {
    let builder = ConnectionBuilder::filename(Some(db_path));
    let mut writer = builder.open_connection(true).await?;
    init_event_db(&mut writer).await?;
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&mut writer)
        .await
        .unwrap_or_default();
    let pool = open_pool(Some(db_path), false).await?;
    let writer = Arc::new(Mutex::new(writer));
    let repo = SqliteEventRepo::new(writer, pool, Arc::new(Metrics::default()));
    Ok((repo, version))
}

fn alert_opts(query: Option<&str>, timeout: Option<u64>) -> elastic::AlertQueryOptions {
    elastic::AlertQueryOptions {
        query_string: query.map(|s| s.to_string()),
        timeout,
        ..Default::default()
    }
}

/// Find an alert group by its (signature_id, src_ip, dest_ip) key.
fn find_group<'a>(
    result: &'a AlertsResult,
    sig: u64,
    src: &str,
    dest: &str,
) -> Option<&'a AggAlert> {
    result.events.iter().find(|a| {
        a.source["alert"]["signature_id"].as_u64() == Some(sig)
            && a.source["src_ip"].as_str().unwrap_or("") == src
            && a.source["dest_ip"].as_str().unwrap_or("") == dest
    })
}

async fn reset_sqlite_alert_group_state(
    repo: &SqliteEventRepo,
    spec: &AlertGroupSpec,
) -> Result<()> {
    let mut query = sqlx::QueryBuilder::new(
        "UPDATE events SET archived = 0, escalated = 0 \
         WHERE json_extract(events.source, '$.event_type') = 'alert' \
         AND json_extract(events.source, '$.alert.signature_id') = ",
    );
    query.push_bind(spec.signature_id as i64);

    match spec.src_ip.as_deref().unwrap_or("") {
        "" => {
            query.push(
                " AND (json_extract(events.source, '$.src_ip') IS NULL \
                 OR json_extract(events.source, '$.src_ip') = '')",
            );
        }
        src_ip => {
            query.push(" AND json_extract(events.source, '$.src_ip') = ");
            query.push_bind(src_ip);
        }
    };

    match spec.dest_ip.as_deref().unwrap_or("") {
        "" => {
            query.push(
                " AND (json_extract(events.source, '$.dest_ip') IS NULL \
                 OR json_extract(events.source, '$.dest_ip') = '')",
            );
        }
        dest_ip => {
            query.push(" AND json_extract(events.source, '$.dest_ip') = ");
            query.push_bind(dest_ip);
        }
    };

    query.push(" AND timestamp >= ");
    query.push_bind(crate::datetime::parse(&spec.min_timestamp, None)?.to_nanos());
    query.push(" AND timestamp <= ");
    query.push_bind(crate::datetime::parse(&spec.max_timestamp, None)?.to_nanos());

    query.build().execute(repo.get_pool()).await?;
    Ok(())
}

async fn check_sqlite_alert_group_exact_fields(repo: &SqliteEventRepo) -> Result<Option<String>> {
    const SENSOR: &str = "evebox-backend-test";
    const SRC_IP: &str = "192.0.2.30";
    const DEST_IP: &str = "198.51.100.40";
    const RRNAME: &str = "discord.com";
    const SNI: &str = "discord.com";
    const DNS_SIGNATURE_ID: u64 = 9_999_997;
    const TLS_SIGNATURE_ID: u64 = 9_999_996;

    let now = DateTime::now();
    let mut sink = repo.get_importer();
    for event in [
        json!({
            "timestamp": now.to_eve(),
            "event_type": "alert",
            "host": SENSOR,
            "src_ip": SRC_IP,
            "dest_ip": DEST_IP,
            "alert": {"signature_id": DNS_SIGNATURE_ID, "signature": "Exact DNS archive test"},
            "dns": {"queries": [
                {"rrname": "keep.example", "rrtype": "A"},
                {"rrname": RRNAME, "rrtype": "A"},
            ]},
            "test_case": "dns-match",
        }),
        json!({
            "timestamp": now.to_eve(),
            "event_type": "alert",
            "host": SENSOR,
            "src_ip": SRC_IP,
            "dest_ip": DEST_IP,
            "alert": {"signature_id": DNS_SIGNATURE_ID, "signature": "Exact DNS archive test"},
            "dns": {"queries": [{"rrname": "keep.example", "rrtype": "A"}]},
            "test_case": "dns-keep",
        }),
        json!({
            "timestamp": now.to_eve(),
            "event_type": "alert",
            "host": SENSOR,
            "src_ip": SRC_IP,
            "dest_ip": DEST_IP,
            "alert": {"signature_id": TLS_SIGNATURE_ID, "signature": "Exact TLS archive test"},
            "tls": {"sni": SNI},
            "test_case": "tls-match",
        }),
        json!({
            "timestamp": now.to_eve(),
            "event_type": "alert",
            "host": SENSOR,
            "src_ip": SRC_IP,
            "dest_ip": DEST_IP,
            "alert": {"signature_id": TLS_SIGNATURE_ID, "signature": "Exact TLS archive test"},
            "tls": {"sni": "keep.example"},
            "test_case": "tls-keep",
        }),
    ] {
        sink.submit(event).await?;
    }
    sink.commit().await?;

    let min_timestamp = (now.clone() - Duration::from_secs(1)).to_rfc3339_utc();
    let max_timestamp = now.to_rfc3339_utc();
    let updated = repo
        .archive_by_alert_group(AlertGroupSpec {
            signature_id: DNS_SIGNATURE_ID,
            src_ip: Some(SRC_IP.to_string()),
            dest_ip: Some(DEST_IP.to_string()),
            sensor: Some(SENSOR.to_string()),
            dns_rrname: Some(RRNAME.to_string()),
            tls_sni: None,
            min_timestamp: min_timestamp.clone(),
            max_timestamp: max_timestamp.clone(),
        })
        .await?;
    if updated != 1 {
        bail!("DNS-constrained alert-group archive updated {updated} events instead of 1");
    }

    let updated = repo
        .archive_by_alert_group(AlertGroupSpec {
            signature_id: TLS_SIGNATURE_ID,
            src_ip: Some(SRC_IP.to_string()),
            dest_ip: Some(DEST_IP.to_string()),
            sensor: Some(SENSOR.to_string()),
            dns_rrname: None,
            tls_sni: Some(SNI.to_string()),
            min_timestamp,
            max_timestamp,
        })
        .await?;
    if updated != 1 {
        bail!("SNI-constrained alert-group archive updated {updated} events instead of 1");
    }

    for (test_case, expected) in [
        ("dns-match", 1_i64),
        ("dns-keep", 0),
        ("tls-match", 1),
        ("tls-keep", 0),
    ] {
        let archived: i64 = sqlx::query_scalar(
            "SELECT archived FROM events WHERE json_extract(source, '$.test_case') = ?",
        )
        .bind(test_case)
        .fetch_one(repo.get_pool())
        .await?;
        if archived != expected {
            bail!(
                "exact alert-group test event {test_case} archived={archived}, expected={expected}"
            );
        }
    }

    Ok(Some("dns=1 tls=1".to_string()))
}

/// Behavioral check of the `is:archived` / `is:escalated` alert search filters
/// against one of the SQLite alert code paths, selected by `timeout`
/// (Some(>0) -> alerts_with_timeout, None -> alerts_group_by).
///
/// Picks a known non-archived alert group, escalates one of its events and
/// archives the whole group, and confirms the group enters and leaves the
/// filtered result sets for is:escalated, is:archived, their negations, and
/// the composition is:escalated -is:archived.
async fn check_sqlite_state_filter(
    repo: &SqliteEventRepo,
    timeout: Option<u64>,
) -> Result<Option<String>> {
    // Pick a clean, non-archived alert group as the ground truth.
    let base = repo
        .alerts(alert_opts(Some("-is:archived"), timeout))
        .await?;
    let group = base
        .events
        .first()
        .ok_or_else(|| anyhow!("no non-archived alert groups to test"))?;
    let sig = group.source["alert"]["signature_id"]
        .as_u64()
        .ok_or_else(|| anyhow!("alert group missing signature_id"))?;
    let src = group.source["src_ip"].as_str().unwrap_or("").to_string();
    let dest = group.source["dest_ip"].as_str().unwrap_or("").to_string();
    let id = group.id.clone();
    let spec = AlertGroupSpec {
        signature_id: sig,
        src_ip: group.source["src_ip"].as_str().map(String::from),
        dest_ip: group.source["dest_ip"].as_str().map(String::from),
        sensor: group.source["host"].as_str().map(String::from),
        dns_rrname: None,
        tls_sni: None,
        min_timestamp: group.metadata.min_timestamp.to_rfc3339_utc(),
        max_timestamp: group.metadata.max_timestamp.to_rfc3339_utc(),
    };

    // Nothing is escalated yet.
    if find_group(
        &repo
            .alerts(alert_opts(Some("is:escalated"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_some()
    {
        bail!("group is escalated before escalation");
    }

    // Escalate one event in the group.
    repo.escalate_event_by_id(&id).await?;

    let escalated = repo
        .alerts(alert_opts(Some("is:escalated"), timeout))
        .await?;
    let hit = find_group(&escalated, sig, &src, &dest)
        .ok_or_else(|| anyhow!("is:escalated did not include the escalated group"))?;
    if hit.metadata.escalated_count < 1 {
        bail!("escalated_count not set on escalated group");
    }
    if find_group(
        &repo
            .alerts(alert_opts(Some("is:escalated -is:archived"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_none()
    {
        bail!("is:escalated -is:archived excluded the escalated, non-archived group");
    }

    // Archive the whole group.
    repo.archive_by_alert_group(spec.clone()).await?;

    if find_group(
        &repo
            .alerts(alert_opts(Some("is:archived"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_none()
    {
        bail!("is:archived did not include the archived group");
    }
    if find_group(
        &repo
            .alerts(alert_opts(Some("-is:archived"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_some()
    {
        bail!("-is:archived included the fully archived group");
    }
    if find_group(
        &repo
            .alerts(alert_opts(Some("is:escalated -is:archived"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_some()
    {
        bail!("is:escalated -is:archived included the now-archived group");
    }
    if find_group(
        &repo
            .alerts(alert_opts(Some("is:escalated"), timeout))
            .await?,
        sig,
        &src,
        &dest,
    )
    .is_none()
    {
        bail!("is:escalated dropped the escalated (archived) group");
    }

    reset_sqlite_alert_group_state(repo, &spec).await?;

    Ok(Some(format!("sig={sig} src={src} dest={dest}")))
}

/// (object, field) pairs where a MAC address may appear in an EVE event.
/// Suricata logs Ethernet MACs under `ether` (as `src_mac`/`dest_mac`, or the
/// `src_macs`/`dest_macs` arrays when several are observed); DHCP and ARP expose
/// their own MAC fields. `ethernet` is kept for any non-Suricata producers.
const MAC_FIELDS: &[(&str, &str)] = &[
    ("ether", "src_mac"),
    ("ether", "dest_mac"),
    ("ether", "src_macs"),
    ("ether", "dest_macs"),
    ("ethernet", "src_mac"),
    ("ethernet", "dest_mac"),
    ("dhcp", "client_mac"),
    ("arp", "src_mac"),
    ("arp", "dest_mac"),
];

/// First MAC address in a JSON value that is a string or an array of strings.
fn value_first_mac(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => a.iter().find_map(|v| v.as_str()).map(String::from),
        _ => None,
    }
}

/// Whether a JSON value (string or array of strings) contains `mac`.
fn value_has_mac(value: &serde_json::Value, mac: &str) -> bool {
    match value {
        serde_json::Value::String(s) => s == mac,
        serde_json::Value::Array(a) => a.iter().any(|v| v.as_str() == Some(mac)),
        _ => false,
    }
}

/// Wrap a value in double quotes for use in a query string, escaping any
/// embedded backslashes and quotes so values containing `:` (such as MAC
/// addresses) survive parsing intact.
fn quote_value(v: &str) -> String {
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Return the first MAC address found in the `_source` of an `events()` result.
fn sample_event_mac(value: &serde_json::Value) -> Option<String> {
    for event in value["events"].as_array()? {
        let source = &event["_source"];
        for (obj, key) in MAC_FIELDS {
            if let Some(mac) = value_first_mac(&source[obj][key]) {
                return Some(mac);
            }
        }
    }
    None
}

/// Whether any event in an `events()` result carries `mac` in a MAC field.
fn event_has_mac(value: &serde_json::Value, mac: &str) -> bool {
    value["events"].as_array().is_some_and(|events| {
        events.iter().any(|event| {
            let source = &event["_source"];
            MAC_FIELDS
                .iter()
                .any(|(obj, key)| value_has_mac(&source[obj][key], mac))
        })
    })
}

fn event_count(value: &serde_json::Value) -> usize {
    value["events"].as_array().map(|a| a.len()).unwrap_or(0)
}

fn event_id(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(String::from)
        .or_else(|| value.as_i64().map(|v| v.to_string()))
        .or_else(|| value.as_u64().map(|v| v.to_string()))
}

fn sample_event_id_and_timestamp(value: &serde_json::Value) -> Result<Option<(String, DateTime)>> {
    let Some(event) = value["events"].as_array().and_then(|events| events.first()) else {
        return Ok(None);
    };
    let id = event_id(&event["_id"]).ok_or_else(|| anyhow!("sample event is missing _id"))?;
    let source = &event["_source"];
    let timestamp = source["timestamp"]
        .as_str()
        .or_else(|| source["@timestamp"].as_str())
        .ok_or_else(|| anyhow!("sample event is missing timestamp"))?;
    let timestamp = crate::datetime::parse(timestamp, None)?;
    Ok(Some((id, timestamp)))
}

fn event_result_has_id(value: &serde_json::Value, id: &str) -> bool {
    value["events"].as_array().is_some_and(|events| {
        events
            .iter()
            .any(|event| event_id(&event["_id"]).as_deref() == Some(id))
    })
}

fn inclusive_date_shortcut_query(ts: &DateTime) -> String {
    let from: DateTime = (ts.datetime - chrono::Duration::seconds(1)).into();
    let to: DateTime = (ts.datetime + chrono::Duration::seconds(1)).into();
    format!(
        "@from:{} @to:{}",
        quote_value(&from.to_rfc3339_utc()),
        quote_value(&to.to_rfc3339_utc())
    )
}

fn exclusive_date_shortcut_query(ts: &DateTime) -> String {
    let ts = quote_value(&ts.to_rfc3339_utc());
    format!("@after:{ts} @before:{ts}")
}

fn verify_date_shortcut_results(
    name: &str,
    start: Instant,
    id: &str,
    ts: &DateTime,
    inclusive: serde_json::Value,
    exclusive: serde_json::Value,
) -> Check {
    if !event_result_has_id(&inclusive, id) {
        return Check::fail(
            name,
            start,
            format!("@from/@to did not include sample event id={id}"),
        );
    }
    let exclusive_count = event_count(&exclusive);
    if exclusive_count != 0 {
        return Check::fail(
            name,
            start,
            format!("@after/@before same-bound query returned {exclusive_count} events"),
        );
    }
    Check::pass(
        name,
        start,
        Some(format!("id={id} ts={}", ts.to_rfc3339_utc())),
    )
}

#[derive(Clone)]
struct AlertSample {
    signature_id: u64,
    src_ip: String,
    dest_ip: String,
    timestamp: DateTime,
}

fn sample_alert_group(result: &AlertsResult) -> Result<Option<AlertSample>> {
    let Some(alert) = result.events.first() else {
        return Ok(None);
    };
    Ok(Some(AlertSample {
        signature_id: alert.source["alert"]["signature_id"]
            .as_u64()
            .ok_or_else(|| anyhow!("sample alert is missing signature_id"))?,
        src_ip: alert.source["src_ip"].as_str().unwrap_or("").to_string(),
        dest_ip: alert.source["dest_ip"].as_str().unwrap_or("").to_string(),
        timestamp: alert.metadata.max_timestamp.clone(),
    }))
}

fn alert_result_has_group(result: &AlertsResult, sample: &AlertSample) -> bool {
    find_group(result, sample.signature_id, &sample.src_ip, &sample.dest_ip).is_some()
}

/// Number of events to scan when sampling for a MAC address.
const MAC_SAMPLE_SIZE: u64 = 1000;

/// Locate a MAC address in the imported events by probing the event types that
/// carry one, newest first. `dhcp` (`dhcp.client_mac`) is the most widely
/// available source; `arp` and `ethernet` (which can be attached to any event
/// type) are also checked. Probing by type avoids missing MACs that fall
/// outside a recent-events window dominated by flow records.
async fn sample_sqlite_event_mac(repo: &SqliteEventRepo) -> Result<Option<String>> {
    for event_type in [Some("dhcp"), Some("arp"), None] {
        let value = repo
            .events(EventQueryParams {
                event_type: event_type.map(str::to_string),
                size: Some(MAC_SAMPLE_SIZE),
                ..Default::default()
            })
            .await?;
        if let Some(mac) = sample_event_mac(&value) {
            return Ok(Some(mac));
        }
    }
    Ok(None)
}

/// Sample a MAC address from an alert event, along with the alert group key
/// (signature_id, src_ip, dest_ip) needed to locate the aggregated group.
fn sample_alert_event_mac(value: &serde_json::Value) -> Option<(String, u64, String, String)> {
    for event in value["events"].as_array()? {
        let source = &event["_source"];
        let Some(sig) = source["alert"]["signature_id"].as_u64() else {
            continue;
        };
        for (obj, key) in MAC_FIELDS {
            if let Some(mac) = value_first_mac(&source[obj][key]) {
                let src = source["src_ip"].as_str().unwrap_or("").to_string();
                let dest = source["dest_ip"].as_str().unwrap_or("").to_string();
                return Some((mac, sig, src, dest));
            }
        }
    }
    None
}

/// Check the `@mac` operator against the SQLite event search path, which matches
/// on the raw event source. Samples a real MAC address, queries for it, and
/// confirms a matching event is returned.
async fn check_sqlite_event_mac(name: &str, repo: &SqliteEventRepo) -> Check {
    let start = Instant::now();
    let mac = match sample_sqlite_event_mac(repo).await {
        Ok(Some(mac)) => mac,
        Ok(None) => return Check::skip(name, "no events expose a MAC field"),
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let query_string = match queryparser::parse(&format!("@mac:{}", quote_value(&mac)), None) {
        Ok(q) => q,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let result = match repo
        .events(EventQueryParams {
            size: Some(100),
            query_string,
            ..Default::default()
        })
        .await
    {
        Ok(v) => v,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    if event_has_mac(&result, &mac) {
        Check::pass(name, start, Some(format!("mac={mac}")))
    } else {
        Check::fail(
            name,
            start,
            format!("@mac:{mac} returned no matching event in the event search path"),
        )
    }
}

async fn check_event_date_shortcuts<F, Fut>(name: &str, mut events: F) -> Check
where
    F: FnMut(EventQueryParams) -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value>>,
{
    let start = Instant::now();
    let sample = match events(EventQueryParams {
        size: Some(1),
        ..Default::default()
    })
    .await
    {
        Ok(v) => v,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let (id, ts) = match sample_event_id_and_timestamp(&sample) {
        Ok(Some(sample)) => sample,
        Ok(None) => return Check::skip(name, "no events to sample"),
        Err(err) => return Check::fail(name, start, err.to_string()),
    };

    let inclusive_query = match queryparser::parse(&inclusive_date_shortcut_query(&ts), None) {
        Ok(q) => q,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let exclusive_query = match queryparser::parse(&exclusive_date_shortcut_query(&ts), None) {
        Ok(q) => q,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };

    let inclusive = match events(EventQueryParams {
        size: Some(100),
        query_string: inclusive_query,
        ..Default::default()
    })
    .await
    {
        Ok(v) => v,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let exclusive = match events(EventQueryParams {
        size: Some(1),
        query_string: exclusive_query,
        ..Default::default()
    })
    .await
    {
        Ok(v) => v,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };

    verify_date_shortcut_results(name, start, &id, &ts, inclusive, exclusive)
}

async fn check_alert_date_shortcuts<F, Fut>(name: &str, mut alerts: F) -> Check
where
    F: FnMut(Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<AlertsResult>>,
{
    let start = Instant::now();
    let sample_result = match alerts(None).await {
        Ok(result) => result,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let sample = match sample_alert_group(&sample_result) {
        Ok(Some(sample)) => sample,
        Ok(None) => return Check::skip(name, "no alert groups to sample"),
        Err(err) => return Check::fail(name, start, err.to_string()),
    };

    let inclusive = match alerts(Some(inclusive_date_shortcut_query(&sample.timestamp))).await {
        Ok(result) => result,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    if !alert_result_has_group(&inclusive, &sample) {
        return Check::fail(
            name,
            start,
            format!(
                "@from/@to did not include sample alert group sig={} src={} dest={}",
                sample.signature_id, sample.src_ip, sample.dest_ip
            ),
        );
    }

    let exclusive = match alerts(Some(exclusive_date_shortcut_query(&sample.timestamp))).await {
        Ok(result) => result,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    if !exclusive.events.is_empty() {
        return Check::fail(
            name,
            start,
            format!(
                "@after/@before same-bound query returned {} alert groups",
                exclusive.events.len()
            ),
        );
    }

    Check::pass(
        name,
        start,
        Some(format!(
            "sig={} src={} dest={} ts={}",
            sample.signature_id,
            sample.src_ip,
            sample.dest_ip,
            sample.timestamp.to_rfc3339_utc()
        )),
    )
}

async fn check_sqlite_alert_date_shortcuts(
    name: &str,
    repo: &SqliteEventRepo,
    timeout: Option<u64>,
) -> Check {
    check_alert_date_shortcuts(name, |query| {
        repo.alerts(alert_opts(query.as_deref(), timeout))
    })
    .await
}

/// Check the `@mac` operator against a SQLite alert search code path, selected
/// by `timeout` (Some(>0) -> alerts_with_timeout, None -> alerts_group_by).
///
/// `@mac` must be honored in the alert search path just as in the event search
/// path. Samples a MAC from a real alert event, then queries for it: PASSES if
/// that alert is returned and FAILS otherwise. Skips only when no alert event
/// exposes a MAC field, since there is then no ground truth to assert against.
async fn check_sqlite_alert_mac(name: &str, repo: &SqliteEventRepo, timeout: Option<u64>) -> Check {
    let start = Instant::now();
    let sample = match repo
        .events(EventQueryParams {
            event_type: Some("alert".to_string()),
            size: Some(MAC_SAMPLE_SIZE),
            ..Default::default()
        })
        .await
    {
        Ok(v) => v,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    let Some((mac, sig, src, dest)) = sample_alert_event_mac(&sample) else {
        return Check::skip(name, "no alert events expose a MAC field");
    };
    let query = format!("@mac:{}", quote_value(&mac));
    let result = match repo.alerts(alert_opts(Some(&query), timeout)).await {
        Ok(r) => r,
        Err(err) => return Check::fail(name, start, err.to_string()),
    };
    if find_group(&result, sig, &src, &dest).is_some() {
        Check::pass(name, start, Some(format!("mac={mac}")))
    } else {
        Check::fail(
            name,
            start,
            format!(
                "@mac:{mac} matched no alert, but an alert with this MAC exists: \
                 @mac is not applied in the SQLite alert search path"
            ),
        )
    }
}

#[derive(Default)]
struct CommonSamples {
    sample_id: Option<String>,
    alert_spec: Option<AlertGroupSpec>,
}

/// Data-dependent checks shared by Elasticsearch/OpenSearch and SQLite.
const COMMON_QUERY_CHECK_NAMES: &[&str] = &[
    "earliest_timestamp",
    "get_event_types",
    "get_sensors",
    "histogram_time",
    "events",
    "events_event_type_filter",
    "events_query_string",
    "events_query_date_shortcuts",
    "alerts_query_date_shortcuts",
    "agg_terms",
    "agg_rare_terms",
    "agg_dns_script",
    "alerts",
    "dhcp_request",
    "dhcp_ack",
    "dns_reverse_lookup",
    "stats_agg",
    "stats_agg_diff",
    "stats_agg_by_sensor",
    "stats_agg_diff_by_sensor",
    "get_event_by_id",
    "escalate_event_by_id",
    "deescalate_event_by_id",
    "archive_event_by_id",
    "comment_event_by_id",
    "archive_by_alert_group",
];

/// SQLite-only checks. These cover SQLite's two alert implementations and
/// SQLite-specific regressions that the single shared datastore path would not
/// distinguish.
const SQLITE_SPECIFIC_QUERY_CHECK_NAMES: &[&str] = &[
    "archive_group_exact_fields",
    "is_state_filter_timeout",
    "is_state_filter_group_by",
    "events_query_mac",
    "alerts_timeout_query_mac",
    "alerts_group_by_query_mac",
    "alerts_timeout_query_date_shortcuts",
];

async fn run_sqlite_report(args: &SqliteArgs) -> Result<Report> {
    // Use the provided database file, or a throwaway temporary directory that
    // is removed when the run completes. `tmp` is declared before `repo` so the
    // repository (and its connection pool) is dropped before the directory is
    // removed.
    let tmp = if args.database.is_none() {
        Some(tempfile::TempDir::new()?)
    } else {
        None
    };
    let db_path = match (&args.database, &tmp) {
        (Some(path), _) => path.clone(),
        (None, Some(tmp)) => tmp.path().join("events.sqlite"),
        (None, None) => unreachable!(),
    };

    info!("Using SQLite database {}", db_path.display());
    let (repo, version) = build_sqlite_repo(&db_path).await?;

    let files = collect_inputs(&args.inputs)?;
    if files.is_empty() {
        return Err(anyhow!("no input files found in {:?}", args.inputs));
    }
    info!(
        "Importing up to {} events from {} files",
        args.limit,
        files.len()
    );

    let mut checks: Vec<Check> = Vec::new();

    let import_start = Instant::now();
    let imported =
        match import_events(EventSink::SQLite(repo.get_importer()), &files, args.limit).await {
            Ok((count, types)) => {
                let summary = types
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                checks.push(Check::pass(
                    "import",
                    import_start,
                    Some(format!("events={count} [{summary}]")),
                ));
                count
            }
            Err(err) => {
                checks.push(Check::fail("import", import_start, err.to_string()));
                0
            }
        };

    if imported > 0 {
        let datastore = EventRepo::SQLite(repo);
        let sqlite_repo = match &datastore {
            EventRepo::SQLite(repo) => repo,
            EventRepo::Elastic(_) => unreachable!(),
        };

        let samples = run_common_read_query_checks(&datastore, &mut checks).await;
        check!(checks, "is_state_filter_timeout", {
            check_sqlite_state_filter(sqlite_repo, Some(3)).await
        });
        check!(checks, "is_state_filter_group_by", {
            check_sqlite_state_filter(sqlite_repo, None).await
        });
        // The `@mac` operator must be honored in the event search path and in
        // both alert search paths.
        checks.push(check_sqlite_event_mac("events_query_mac", sqlite_repo).await);
        checks.push(check_sqlite_alert_mac("alerts_timeout_query_mac", sqlite_repo, Some(3)).await);
        checks.push(check_sqlite_alert_mac("alerts_group_by_query_mac", sqlite_repo, None).await);
        checks.push(
            check_sqlite_alert_date_shortcuts(
                "alerts_timeout_query_date_shortcuts",
                sqlite_repo,
                Some(3),
            )
            .await,
        );
        run_common_mutation_checks(&datastore, &mut checks, true, samples).await;
        check!(checks, "archive_group_exact_fields", {
            check_sqlite_alert_group_exact_fields(sqlite_repo).await
        });
    } else {
        for name in COMMON_QUERY_CHECK_NAMES {
            checks.push(Check::skip(name, "no events imported"));
        }
        for name in SQLITE_SPECIFIC_QUERY_CHECK_NAMES {
            checks.push(Check::skip(name, "no events imported"));
        }
    }

    if let Some(path) = &args.database {
        info!("Keeping SQLite database {}", path.display());
    }

    let passed = checks.iter().filter(|c| c.status == Status::Pass).count();
    let failed = checks.iter().filter(|c| c.status == Status::Fail).count();
    let known = checks.iter().filter(|c| c.status == Status::Known).count();
    let skipped = checks.iter().filter(|c| c.status == Status::Skip).count();

    Ok(Report {
        distribution: "sqlite".to_string(),
        version,
        tagline: None,
        supported: true,
        mode: "import",
        index: db_path.display().to_string(),
        imported,
        warnings: Vec::new(),
        checks,
        passed,
        failed,
        known,
        skipped,
    })
}

/// Names of the data-modifying checks, skipped in read-only (`--existing`) mode.
const MUTATION_CHECK_NAMES: &[&str] = &[
    "escalate_event_by_id",
    "deescalate_event_by_id",
    "archive_event_by_id",
    "comment_event_by_id",
    "archive_by_alert_group",
];

/// Run read-only datastore checks shared by Elasticsearch/OpenSearch and
/// SQLite. Returns sampled identifiers needed by the later mutation checks.
async fn run_common_read_query_checks(repo: &EventRepo, checks: &mut Vec<Check>) -> CommonSamples {
    let mut samples = CommonSamples::default();

    // earliest_timestamp — also reused for the stats time range.
    let earliest = {
        let start = Instant::now();
        match repo.earliest_timestamp().await {
            Ok(ts) => {
                let detail = ts
                    .as_ref()
                    .map(|t| format!("earliest={}", t.to_rfc3339_utc()));
                checks.push(Check::pass("earliest_timestamp", start, detail));
                ts
            }
            Err(err) => {
                checks.push(Check::fail("earliest_timestamp", start, err.to_string()));
                None
            }
        }
    };

    check!(checks, "get_event_types", {
        let types = match repo {
            EventRepo::Elastic(repo) => repo.get_event_types().await?,
            EventRepo::SQLite(repo) => repo.get_event_types(Vec::new()).await?,
        };
        Ok(Some(format!("types={}", types.len())))
    });

    check!(checks, "get_sensors", {
        let sensors = match repo {
            EventRepo::Elastic(repo) => repo.get_sensors().await?,
            EventRepo::SQLite(repo) => repo.get_sensors().await?,
        };
        Ok(Some(format!("sensors={}", sensors.len())))
    });

    check!(checks, "histogram_time", {
        let buckets = match repo {
            EventRepo::Elastic(repo) => repo.histogram_time(None, &[]).await?,
            EventRepo::SQLite(repo) => repo.histogram_time(None, &[]).await?,
        };
        Ok(Some(format!("buckets={}", buckets.len())))
    });

    // events — also captures a sample document id for the id-based checks.
    let sample_id = {
        let start = Instant::now();
        let params = EventQueryParams {
            size: Some(10),
            ..Default::default()
        };
        match repo.events(params).await {
            Ok(value) => {
                let count = value["events"].as_array().map(|a| a.len()).unwrap_or(0);
                let id = value["events"]
                    .as_array()
                    .and_then(|events| events.first())
                    .and_then(|event| event_id(&event["_id"]));
                checks.push(Check::pass(
                    "events",
                    start,
                    Some(format!("events={count}")),
                ));
                id
            }
            Err(err) => {
                checks.push(Check::fail("events", start, err.to_string()));
                None
            }
        }
    };

    check!(checks, "events_event_type_filter", {
        let params = EventQueryParams {
            size: Some(10),
            event_type: Some("alert".to_string()),
            ..Default::default()
        };
        let value = repo.events(params).await?;
        let count = value["events"].as_array().map(|a| a.len()).unwrap_or(0);
        Ok(Some(format!("events={count}")))
    });

    // Free-text query_string. On indices with very wide mappings this hits a
    // known EveBox limitation (field-less query_string expansion exceeds the
    // engine field limit), which is recorded as a known limitation, not a fail.
    {
        let start = Instant::now();
        let result: Result<Option<String>> = async {
            let params = EventQueryParams {
                size: Some(10),
                query_string: queryparser::parse("dns", None)?,
                ..Default::default()
            };
            let value = repo.events(params).await?;
            let count = value["events"].as_array().map(|a| a.len()).unwrap_or(0);
            Ok(Some(format!("events={count}")))
        }
        .await;
        checks.push(Check::finish_lenient(
            "events_query_string",
            start,
            result,
            &[FIELD_EXPANSION_LIMITATION],
        ));
    }

    checks.push(
        check_event_date_shortcuts("events_query_date_shortcuts", |params| repo.events(params))
            .await,
    );
    let auto_archive = Arc::new(RwLock::new(AutoArchive::default()));
    checks.push(
        check_alert_date_shortcuts("alerts_query_date_shortcuts", |query| {
            repo.alerts(
                elastic::AlertQueryOptions {
                    query_string: query,
                    ..Default::default()
                },
                auto_archive.clone(),
            )
        })
        .await,
    );

    check!(checks, "agg_terms", {
        let rows = repo.agg("src_ip", 10, "desc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "agg_rare_terms", {
        let rows = repo.agg("src_ip", 10, "asc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    // dns.rrname exercises datastore-specific nested DNS name handling.
    check!(checks, "agg_dns_script", {
        let rows = repo.agg("dns.rrname", 10, "desc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    // alerts — the big nested aggregation; also captures an alert group.
    {
        let start = Instant::now();
        let auto_archive = Arc::new(RwLock::new(AutoArchive::default()));
        match repo
            .alerts(elastic::AlertQueryOptions::default(), auto_archive)
            .await
        {
            Ok(result) => {
                let spec = result.events.first().map(|alert| AlertGroupSpec {
                    signature_id: alert.source["alert"]["signature_id"].as_u64().unwrap_or(0),
                    src_ip: alert.source["src_ip"].as_str().map(String::from),
                    dest_ip: alert.source["dest_ip"].as_str().map(String::from),
                    sensor: alert.source["host"].as_str().map(String::from),
                    dns_rrname: None,
                    tls_sni: None,
                    min_timestamp: alert.metadata.min_timestamp.to_rfc3339_utc(),
                    max_timestamp: alert.metadata.max_timestamp.to_rfc3339_utc(),
                });
                checks.push(Check::pass(
                    "alerts",
                    start,
                    Some(format!("alert_groups={}", result.events.len())),
                ));
                samples.alert_spec = spec;
            }
            Err(err) => {
                checks.push(Check::fail("alerts", start, err.to_string()));
            }
        }
    };

    check!(checks, "dhcp_request", {
        let rows = match repo {
            EventRepo::Elastic(repo) => repo.dhcp_request(None, None).await?,
            EventRepo::SQLite(repo) => repo.dhcp_request(None, None).await?,
        };
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "dhcp_ack", {
        let rows = match repo {
            EventRepo::Elastic(repo) => repo.dhcp_ack(None, None).await?,
            EventRepo::SQLite(repo) => repo.dhcp_ack(None, None).await?,
        };
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "dns_reverse_lookup", {
        let value = match repo {
            EventRepo::Elastic(repo) => {
                repo.dns_reverse_lookup(None, None, "10.0.0.1".to_string(), "10.0.0.2".to_string())
                    .await?
            }
            EventRepo::SQLite(repo) => {
                repo.dns_reverse_lookup(None, None, "10.0.0.1".to_string(), "10.0.0.2".to_string())
                    .await?
            }
        };
        let _ = value;
        Ok(None)
    });

    // Stats reports: date_histogram + max + derivative pipeline aggregation.
    let stats_end = DateTime::now();
    let stats_start = match &earliest {
        Some(ts) => ts.clone(),
        None => stats_end.clone() - Duration::from_secs(365 * 24 * 60 * 60),
    };
    let stats_params = StatsAggQueryParams {
        field: "stats.uptime".to_string(),
        sensor_name: None,
        start_time: stats_start,
        end_time: stats_end,
    };

    check!(checks, "stats_agg", {
        let _ = repo.stats_agg(&stats_params).await?;
        Ok(None)
    });
    check!(checks, "stats_agg_diff", {
        let _ = repo.stats_agg_diff(&stats_params).await?;
        Ok(None)
    });
    check!(checks, "stats_agg_by_sensor", {
        let _ = repo.stats_agg_by_sensor(&stats_params).await?;
        Ok(None)
    });
    check!(checks, "stats_agg_diff_by_sensor", {
        let _ = repo.stats_agg_diff_by_sensor(&stats_params).await?;
        Ok(None)
    });

    // get_event_by_id (read-only, term _id) — runs in both modes.
    match &sample_id {
        Some(id) => {
            check!(checks, "get_event_by_id", {
                let event = repo.get_event_by_id(id.clone()).await?;
                Ok(Some(format!("found={}", event.is_some())))
            });
        }
        None => checks.push(Check::skip("get_event_by_id", "no sample event id")),
    }

    samples.sample_id = sample_id;
    samples
}

/// Run shared data-modifying checks. When `mutate` is false (read-only
/// `--existing` mode) the checks are skipped so existing data is never touched.
async fn run_common_mutation_checks(
    repo: &EventRepo,
    checks: &mut Vec<Check>,
    mutate: bool,
    samples: CommonSamples,
) {
    // Data-modifying checks (_update_by_query painless). Skipped entirely in
    // read-only mode so existing data is never modified.
    if !mutate {
        for name in MUTATION_CHECK_NAMES {
            checks.push(Check::skip(name, "read-only mode (existing data)"));
        }
        return;
    }

    // Id-based mutation checks.
    match &samples.sample_id {
        Some(id) => {
            check!(checks, "escalate_event_by_id", {
                repo.escalate_event_by_id(id).await?;
                Ok(None)
            });
            check!(checks, "deescalate_event_by_id", {
                repo.deescalate_event_by_id(id).await?;
                Ok(None)
            });
            check!(checks, "archive_event_by_id", {
                repo.archive_event_by_id(id).await?;
                Ok(None)
            });
            let session = Arc::new(Session::anonymous(Some("backend-test".to_string())));
            check!(checks, "comment_event_by_id", {
                repo.comment_event_by_id(id, "backend test".to_string(), session.clone())
                    .await?;
                Ok(None)
            });
        }
        None => {
            for name in [
                "escalate_event_by_id",
                "deescalate_event_by_id",
                "archive_event_by_id",
                "comment_event_by_id",
            ] {
                checks.push(Check::skip(name, "no sample event id"));
            }
        }
    }

    // Alert-group mutation (build_alert_group_filter + _update_by_query).
    match samples.alert_spec {
        Some(spec) => {
            let start = Instant::now();
            match repo.archive_by_alert_group(spec).await {
                Ok(n) => checks.push(Check::pass(
                    "archive_by_alert_group",
                    start,
                    Some(format!("updated={n}")),
                )),
                Err(err) => checks.push(Check::fail(
                    "archive_by_alert_group",
                    start,
                    err.to_string(),
                )),
            }
        }
        None => checks.push(Check::skip("archive_by_alert_group", "no alerts in sample")),
    }
}

/// Import up to `limit` events, reading from `files` round-robin so the sample
/// gets a mix of event types rather than all of one type from a single file.
async fn import_events(
    mut sink: EventSink,
    files: &[PathBuf],
    limit: usize,
) -> Result<(usize, BTreeMap<String, usize>)> {
    let chain = EveFilterChain::with_defaults();
    let mut readers: Vec<EveReader> = files.iter().map(|f| EveReader::new(f.clone())).collect();
    let mut exhausted = vec![false; readers.len()];
    let mut remaining = readers.len();
    let mut count = 0usize;
    let mut types: BTreeMap<String, usize> = BTreeMap::new();

    'outer: while remaining > 0 && count < limit {
        for i in 0..readers.len() {
            if exhausted[i] {
                continue;
            }
            match readers[i].next_record() {
                Ok(Some(mut event)) => {
                    // submit() panics on events without a parseable timestamp.
                    if event.datetime().is_none() {
                        continue;
                    }
                    chain.run(&mut event);
                    if let Some(event_type) = event["event_type"].as_str() {
                        *types.entry(event_type.to_string()).or_default() += 1;
                    }
                    sink.submit(event).await?;
                    count += 1;
                    if sink.pending() >= 1000 {
                        sink.commit().await?;
                    }
                    if count >= limit {
                        break 'outer;
                    }
                }
                Ok(None) => {
                    exhausted[i] = true;
                    remaining -= 1;
                }
                Err(err) => {
                    warn!("Error reading {}: {}", readers[i].filename.display(), err);
                    exhausted[i] = true;
                    remaining -= 1;
                }
            }
        }
    }

    if sink.pending() > 0 {
        sink.commit().await?;
    }

    Ok((count, types))
}

/// Expand input paths into a sorted list of EVE files. Directories are searched
/// recursively for files whose name ends in `.json` (this excludes rotated
/// files like `eve.1.json-20260623`).
fn collect_inputs(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = paths.to_vec();
    while let Some(path) = stack.pop() {
        let meta =
            std::fs::metadata(&path).map_err(|err| anyhow!("{}: {}", path.display(), err))?;
        if meta.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                stack.push(entry?.path());
            }
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".json"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

async fn refresh(client: &elastic::Client, base: &str) -> Result<()> {
    client
        .post(&format!("{base}*/_refresh"))?
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Delete the test indices and the field-limit template created for the run.
async fn cleanup(client: &elastic::Client, base: &str) -> Result<usize> {
    let mut deleted = 0;
    for index in client.get_indices_pattern(&format!("{base}*")).await? {
        let status = client.delete_index(&index.index).await?;
        if status.is_success() {
            deleted += 1;
        } else {
            warn!("Deleting index {} returned {}", index.index, status);
        }
    }
    // Best-effort template cleanup.
    let _ = client.delete(&format!("_template/{base}"))?.send().await;
    Ok(deleted)
}
