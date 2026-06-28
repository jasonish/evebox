// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! `evebox test elastic` — Elasticsearch/OpenSearch compatibility test.
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
//! The companion harness `docker/tests/compat/run.sh` runs this command against
//! a matrix of container versions.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use clap::{Command, CommandFactory, FromArgMatches, Parser, Subcommand};
use serde::Serialize;
use tracing::{info, warn};

use crate::datetime::DateTime;
use crate::elastic::{self, ClientBuilder, ElasticEventRepo};
use crate::eve::Eve;
use crate::eve::filters::EveFilterChain;
use crate::eve::reader::EveReader;
use crate::eventrepo::{EventQueryParams, StatsAggQueryParams};
use crate::queryparser;
use crate::server::api::AlertGroupSpec;
use crate::server::autoarchive::AutoArchive;
use crate::server::session::Session;

#[derive(Parser, Debug)]
#[command(name = "test", about = "Test datastore compatibility")]
pub(crate) struct TestArgs {
    #[command(subcommand)]
    command: TestCommand,
}

#[derive(Subcommand, Debug)]
enum TestCommand {
    /// Test Elasticsearch/OpenSearch compatibility against a corpus of EVE events
    #[command(visible_alias = "opensearch")]
    Elastic(Args),
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
    /// index is created and deleted by the test (default: evebox-compat-test).
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

/// Default index prefix for import mode (a unique per-run suffix is appended).
const DEFAULT_IMPORT_INDEX: &str = "evebox-compat-test";

/// Default existing-index prefix for --existing mode (EveBox's own default).
const DEFAULT_EXISTING_INDEX: &str = "logstash";

pub fn command() -> Command {
    TestArgs::command()
}

pub async fn main(args: &clap::ArgMatches) -> Result<()> {
    let args = TestArgs::from_arg_matches(args)?;
    match args.command {
        TestCommand::Elastic(args) => run_elastic(&args).await,
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
    checks: Vec<Check>,
    passed: usize,
    failed: usize,
    known: usize,
    skipped: usize,
}

impl Report {
    fn print_human(&self) {
        println!("EveBox datastore compatibility test");
        println!("  Server:   {} {}", self.distribution, self.version);
        if let Some(tagline) = &self.tagline {
            println!("  Tagline:  {tagline}");
        }
        if !self.supported {
            println!("  WARNING:  this version is below EveBox's supported floor");
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
    info!("Connected to {} {}", distribution, version);
    if !supported {
        warn!(
            "{} {} is below EveBox's supported floor; testing anyway",
            distribution, version
        );
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
    let repo = ElasticEventRepo::new(base.to_string(), index_pattern, client.clone(), false);

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
        match import_events(&repo, &files, args.limit).await {
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
        run_query_checks(&repo, &mut checks, !args.existing).await;
    } else {
        for name in QUERY_CHECK_NAMES {
            checks.push(Check::skip(name, "no events imported"));
        }
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
        checks,
        passed,
        failed,
        known,
        skipped,
    })
}

/// Names of the data-dependent checks, used to emit skips when nothing was
/// imported. Kept in sync with [`run_query_checks`].
const QUERY_CHECK_NAMES: &[&str] = &[
    "earliest_timestamp",
    "get_event_types",
    "get_sensors",
    "histogram_time",
    "events",
    "events_event_type_filter",
    "events_query_string",
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

/// Names of the data-modifying checks, skipped in read-only (`--existing`) mode.
const MUTATION_CHECK_NAMES: &[&str] = &[
    "escalate_event_by_id",
    "deescalate_event_by_id",
    "archive_event_by_id",
    "comment_event_by_id",
    "archive_by_alert_group",
];

/// Run every data-dependent check against the populated index. When `mutate` is
/// false (read-only `--existing` mode) the data-modifying checks are skipped so
/// existing data is never touched.
async fn run_query_checks(repo: &ElasticEventRepo, checks: &mut Vec<Check>, mutate: bool) {
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
        let types = repo.get_event_types().await?;
        Ok(Some(format!("types={}", types.len())))
    });

    check!(checks, "get_sensors", {
        let sensors = repo.get_sensors().await?;
        Ok(Some(format!("sensors={}", sensors.len())))
    });

    check!(checks, "histogram_time", {
        let buckets = repo.histogram_time(None, &[]).await?;
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
                let id = value["events"][0]["_id"].as_str().map(String::from);
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

    check!(checks, "agg_terms", {
        let rows = repo.agg("src_ip", 10, "desc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "agg_rare_terms", {
        let rows = repo.agg("src_ip", 10, "asc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    // dns.rrname maps to a painless script source on the elastic side.
    check!(checks, "agg_dns_script", {
        let rows = repo.agg("dns.rrname", 10, "desc", vec![]).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    // alerts — the big nested aggregation; also captures an alert group.
    let alert_spec = {
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
                    min_timestamp: alert.metadata.min_timestamp.to_rfc3339_utc(),
                    max_timestamp: alert.metadata.max_timestamp.to_rfc3339_utc(),
                });
                checks.push(Check::pass(
                    "alerts",
                    start,
                    Some(format!("alert_groups={}", result.events.len())),
                ));
                spec
            }
            Err(err) => {
                checks.push(Check::fail("alerts", start, err.to_string()));
                None
            }
        }
    };

    check!(checks, "dhcp_request", {
        let rows = repo.dhcp_request(None, None).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "dhcp_ack", {
        let rows = repo.dhcp_ack(None, None).await?;
        Ok(Some(format!("rows={}", rows.len())))
    });

    check!(checks, "dns_reverse_lookup", {
        let value = repo
            .dns_reverse_lookup(None, None, "10.0.0.1".to_string(), "10.0.0.2".to_string())
            .await?;
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

    // Data-modifying checks (_update_by_query painless). Skipped entirely in
    // read-only mode so existing data is never modified.
    if !mutate {
        for name in MUTATION_CHECK_NAMES {
            checks.push(Check::skip(name, "read-only mode (existing data)"));
        }
        return;
    }

    // Id-based mutation checks.
    match &sample_id {
        Some(id) => {
            check!(checks, "escalate_event_by_id", {
                let n = repo.escalate_event_by_id(id).await?;
                Ok(Some(format!("updated={n}")))
            });
            check!(checks, "deescalate_event_by_id", {
                repo.deescalate_event_by_id(id).await?;
                Ok(None)
            });
            check!(checks, "archive_event_by_id", {
                let n = repo.archive_event_by_id(id).await?;
                Ok(Some(format!("updated={n}")))
            });
            let session = Arc::new(Session::anonymous(Some("compat-test".to_string())));
            check!(checks, "comment_event_by_id", {
                let n = repo
                    .comment_event_by_id(id, "compat test".to_string(), session.clone())
                    .await?;
                Ok(Some(format!("updated={n}")))
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
    match alert_spec {
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
    repo: &ElasticEventRepo,
    files: &[PathBuf],
    limit: usize,
) -> Result<(usize, BTreeMap<String, usize>)> {
    let chain = EveFilterChain::with_defaults();
    let mut sink = repo
        .get_importer()
        .ok_or_else(|| anyhow!("event importer unavailable (ECS mode is not supported)"))?;

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
