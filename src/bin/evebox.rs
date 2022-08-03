// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

#![allow(clippy::redundant_field_names)]

use clap::{Arg, Command, IntoApp};
use evebox::logger;
use evebox::prelude::*;
use evebox::version;

#[tokio::main]
async fn main() {
    if let Err(err) = _main().await {
        error!("{}", err);
        std::process::exit(1);
    }
}

async fn _main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = clap::Command::new("EveBox")
        .version(std::env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .multiple_occurrences(true)
                .global(true)
                .help("Increase verbosity"),
        )
        .arg(
            Arg::new("data-directory")
                .long("data-directory")
                .short('D')
                .takes_value(true)
                .value_name("DIR")
                .help("Data directory")
                .global(true),
        )
        .subcommand(clap::Command::new("version").about("Display version"));

    let sqlite_import = Command::new("sqlite-import")
        .about("Import to SQLite")
        .arg(
            Arg::new("oneshot")
                .long("oneshot")
                .help("One shot mode (exit on EOF"),
        )
        .arg(Arg::new("end").long("end").help("Start at end of file"))
        .arg(Arg::new("INPUT").required(true).index(1));

    let server = clap::Command::new("server")
        .about("EveBox Server")
        .arg(
            clap::Arg::new("config")
                .long("config")
                .short('c')
                .takes_value(true)
                .value_name("FILE")
                .help("Configuration filename"),
        )
        .arg(
            clap::Arg::new("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .takes_value(true)
                .default_value("127.0.0.1")
                .env("EVEBOX_HTTP_HOST")
                .help("Hostname/IP address to bind to"),
        )
        .arg(
            clap::Arg::new("http.port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .takes_value(true)
                .default_value("5636")
                .env("EVEBOX_HTTP_PORT")
                .help("Port to bind to"),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.url")
                .short('e')
                .long("elasticsearch")
                .takes_value(true)
                .value_name("URL")
                .default_value("http://localhost:9200")
                .env("EVEBOX_ELASTICSEARCH_URL")
                .help("Elastic Search URL"),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.index")
                .short('i')
                .long("index")
                .takes_value(true)
                .default_value("logstash")
                .value_name("INDEX")
                .help("Elastic Search index prefix"),
        )
        .arg(
            Arg::new("database.elasticsearch.no-index-suffix")
                .long("no-index-suffix")
                .takes_value(false)
                .help("Do not add a suffix to the index name"),
        )
        .arg(
            Arg::new("database.elasticsearch.ecs")
                .long("ecs")
                .help("Enable Elastic ECS support"),
        )
        .arg(
            Arg::new("database.type")
                .long("database")
                .aliases(&["datastore"])
                .takes_value(true)
                .value_name("DATABASE")
                .default_value("elasticsearch")
                .help("Database type"),
        )
        .arg(
            Arg::new("sqlite-filename")
                .long("sqlite-filename")
                .takes_value(true)
                .value_name("FILENAME")
                .default_value("events.sqlite")
                .help("SQLite events filename"),
        )
        .arg(
            clap::Arg::new("no-check-certificate")
                .short('k')
                .long("no-check-certificate")
                .help("Disable TLS certificate validation"),
        )
        .arg(
            Arg::new("http.request-logging")
                .long("http-request-logging")
                .env("EVEBOX_HTTP_REQUEST_LOGGING")
                .hide_env(true)
                .help("Enable HTTP access logging"),
        )
        .arg(
            Arg::new("http.tls.enabled")
                .long("tls")
                .help("Enable TLS")
                .env("EVEBOX_HTTP_TLS_ENABLED")
                .hide_env(true),
        )
        .arg(
            Arg::new("http.tls.certificate")
                .long("tls-cert")
                .takes_value(true)
                .value_name("FILENAME")
                .help("TLS certificate filename"),
        )
        .arg(
            Arg::new("http.tls.key")
                .long("tls-key")
                .takes_value(true)
                .value_name("FILENAME")
                .help("TLS key filename"),
        )
        .arg(
            Arg::new("input.filename")
                .long("input")
                .takes_value(true)
                .value_name("FILENAME")
                .help("Input Eve file to read"),
        )
        .arg(
            Arg::new("end")
                .long("end")
                .help("Read from end (tail) of input file"),
        )
        .arg(Arg::new("input-start").long("input-start").hide(true));

    let oneshot = Command::new("oneshot")
        .about("Import a single eve.json and review in EveBox")
        .arg(
            Arg::new("limit")
                .long("limit")
                .takes_value(true)
                .help("Limit the number of events read"),
        )
        .arg(
            Arg::new("no-open")
                .long("no-open")
                .help("Don't open browser"),
        )
        .arg(
            Arg::new("no-wait")
                .long("no-wait")
                .help("Don't wait for events to load"),
        )
        .arg(
            Arg::new("database-filename")
                .long("database-filename")
                .takes_value(true)
                .default_value("./oneshot.sqlite")
                .help("Database filename"),
        )
        // --host, but keep th name as http.host to be campatible with the
        // EVEBOX_HTTP_HOST environment variable.
        .arg(
            clap::Arg::new("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .takes_value(true)
                .default_value("127.0.0.1")
                .help("Hostname/IP address to bind to"),
        )
        .arg(Arg::new("INPUT").required(true).index(1));

    let elastic_import = Command::new("elastic-import")
        .about("Import to Elastic Search")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .takes_value(true)
                .help("Configuration file"),
        )
        .arg(
            Arg::new("oneshot")
                .long("oneshot")
                .help("One shot mode (exit on EOF)"),
        )
        .arg(Arg::new("end").long("end").help("Start at end of file"))
        .arg(
            clap::Arg::new("elasticsearch")
                .short('e')
                .long("elasticsearch")
                .takes_value(true)
                .default_value("http://localhost:9200")
                .hide_default_value(true)
                .help("Elastic Search URL"),
        )
        .arg(
            Arg::new("index")
                .long("index")
                .takes_value(true)
                .default_value("logstash")
                .hide_default_value(true)
                .help("Elastic Search index prefix"),
        )
        .arg(
            Arg::new("no-index-suffix")
                .long("no-index-suffix")
                .takes_value(false)
                .help("Do not add a suffix to the index name"),
        )
        .arg(
            Arg::new("bookmark")
                .long("bookmark")
                .help("Enable bookmarking"),
        )
        .arg(
            Arg::new("bookmark-filename")
                .long("bookmark-filename")
                .takes_value(true)
                .default_value("")
                .hide_default_value(true)
                .help("Bookmark filename"),
        )
        .arg(
            Arg::new("bookmark-dir")
                .long("bookmark-dir")
                .takes_value(true)
                .default_value(".")
                .hide_default_value(true)
                .help("Bookmark directory"),
        )
        .arg(
            Arg::new("stdout")
                .long("stdout")
                .help("Print events to stdout"),
        )
        .arg(
            Arg::new("username")
                .long("username")
                .short('u')
                .takes_value(true)
                .help("Elasticsearch username"),
        )
        .arg(
            Arg::new("password")
                .long("password")
                .short('p')
                .takes_value(true)
                .help("Elasticsearch password"),
        )
        .arg(
            clap::Arg::new(evebox::commands::elastic_import::NO_CHECK_CERTIFICATE)
                .short('k')
                .long("no-check-certificate")
                .help("Disable TLS certificate validation"),
        )
        .arg(
            Arg::new("geoip.disabled")
                .long("no-geoip")
                .help("Disable GeoIP"),
        )
        .arg(
            Arg::new("geoip.database-filename")
                .long("geoip-database")
                .takes_value(true)
                .value_name("filename")
                .help("GeoIP database filename"),
        )
        .arg(
            Arg::new("input")
                .multiple_values(true)
                .value_name("INPUT")
                .index(1),
        );

    let mut parser = parser
        .subcommand(server)
        .subcommand(elastic_import)
        .subcommand(oneshot)
        .subcommand(evebox::commands::agent::command())
        .subcommand(sqlite_import)
        .subcommand(evebox::commands::config::config_subcommand())
        .subcommand(evebox::commands::print::command())
        .subcommand(evebox::commands::elastic::main::main_options());
    let matches = parser.clone().get_matches();

    // Initialize logging.
    let log_level = {
        let verbosity = matches.occurrences_of("verbose");
        if verbosity > 1 {
            tracing::Level::TRACE
        } else if verbosity > 0 {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    };
    logger::init_logger(log_level);
    logger::init_stdlog();

    let rc: anyhow::Result<()> = match matches.subcommand() {
        Some(("server", args)) => {
            evebox::server::main(args).await?;
            Ok(())
        }
        Some(("version", _)) => {
            version::print_version();
            Ok(())
        }
        Some(("sqlite-import", args)) => evebox::commands::sqlite_import::main(args).await,
        Some(("elastic-import", args)) => {
            if let Err(err) = evebox::commands::elastic_import::main(args).await {
                error!("{}", err);
                std::process::exit(1);
            }
            Ok(())
        }
        Some(("oneshot", args)) => evebox::commands::oneshot::main(args).await,
        Some(("agent", args)) => evebox::commands::agent::main(args).await,
        Some(("config", args)) => evebox::commands::config::main(args),
        Some(("print", args)) => evebox::commands::print::main(args),
        Some(("elastic", args)) => evebox::commands::elastic::main::main(args).await,
        _ => {
            parser.print_help().ok();
            println!();
            std::process::exit(1);
        }
    };
    if let Err(err) = rc {
        error!("error: {}", err);
        std::process::exit(1);
    } else {
        Ok(())
    }
}
