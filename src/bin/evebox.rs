// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

#![allow(clippy::redundant_field_names)]

use clap::{value_parser, ArgAction};
use clap::{Arg, Command};
use evebox::logger;
use evebox::prelude::*;
use evebox::version;

fn main() {
    logger::init_offset();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if let Err(err) = evebox_main().await {
            error!("{}", err);
            std::process::exit(1);
        }
    })
}

async fn evebox_main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(command) = std::env::args().nth(1) {
        if command == "elastic-import" {
            logger::init_logger(tracing::Level::INFO).unwrap();
            error!("elastic-import has been deprecated. The Agent can now be used to send events to Elasticsearch.");
            std::process::exit(1);
        }
    }

    let parser = clap::Command::new("EveBox")
        .version(std::env!("CARGO_PKG_VERSION"))
        .color(clap::ColorChoice::Always)
        .arg(
            Arg::new("verbose")
                .action(clap::ArgAction::Count)
                .long("verbose")
                .short('v')
                .global(true)
                .help("Increase verbosity"),
        )
        .arg(
            Arg::new("data-directory")
                .long("data-directory")
                .short('D')
                .action(ArgAction::Set)
                .value_name("DIR")
                .help("Data directory")
                .env("EVEBOX_DATA_DIRECTORY")
                .global(true),
        )
        .subcommand(clap::Command::new("version").about("Display version"));

    let server = clap::Command::new("server")
        .about("EveBox Server")
        .arg(
            clap::Arg::new("config")
                .long("config")
                .short('c')
                .action(ArgAction::Set)
                .value_name("FILE")
                .help("Configuration filename"),
        )
        .arg(
            clap::Arg::new("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .action(ArgAction::Set)
                .default_value("127.0.0.1")
                .env("EVEBOX_HTTP_HOST")
                .help("Hostname/IP address to bind to"),
        )
        .arg(
            clap::Arg::new("http.port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .action(ArgAction::Set)
                .default_value("5636")
                .env("EVEBOX_HTTP_PORT")
                .value_parser(value_parser!(u16))
                .help("Port to bind to"),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.url")
                .short('e')
                .long("elasticsearch")
                .action(ArgAction::Set)
                .value_name("URL")
                .default_value("http://localhost:9200")
                .env("EVEBOX_ELASTICSEARCH_URL")
                .help("Elastic Search URL"),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.username")
                .long("elasticsearch-username")
                .action(ArgAction::Set)
                .value_name("NAME")
                .env("EVEBOX_ELASTICSEARCH_USERNAME")
                .hide(true),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.password")
                .long("elasticsearch-password")
                .action(ArgAction::Set)
                .value_name("PASS")
                .env("EVEBOX_ELASTICSEARCH_PASSWORD")
                .hide(true),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.index")
                .short('i')
                .long("index")
                .action(ArgAction::Set)
                .default_value("logstash")
                .value_name("INDEX")
                .help("Elastic Search index prefix"),
        )
        .arg(
            Arg::new("database.elasticsearch.no-index-suffix")
                .action(clap::ArgAction::SetTrue)
                .long("no-index-suffix")
                .help("Do not add a suffix to the index name"),
        )
        .arg(
            Arg::new("database.elasticsearch.ecs")
                .action(ArgAction::SetTrue)
                .long("ecs")
                .env("EVEBOX_ELASTICSEARCH_ECS")
                .help("Enable Elastic ECS support"),
        )
        .arg(
            Arg::new("database.type")
                .long("database")
                .aliases(["datastore"])
                .action(ArgAction::Set)
                .value_name("DATABASE")
                .default_value("elasticsearch")
                .help("Database type"),
        )
        .arg(
            clap::Arg::new("no-check-certificate")
                .action(ArgAction::SetTrue)
                .short('k')
                .long("no-check-certificate")
                .help("Disable TLS certificate validation"),
        )
        .arg(
            Arg::new("http.request-logging")
                .action(ArgAction::SetTrue)
                .long("http-request-logging")
                .env("EVEBOX_HTTP_REQUEST_LOGGING")
                .hide_env(true)
                .help("Enable HTTP access logging"),
        )
        .arg(
            Arg::new("http.tls.enabled")
                .action(ArgAction::SetTrue)
                .long("tls")
                .help("Enable TLS")
                .env("EVEBOX_HTTP_TLS_ENABLED")
                .hide_env(true),
        )
        .arg(
            Arg::new("http.tls.certificate")
                .long("tls-cert")
                .action(ArgAction::Set)
                .value_name("FILENAME")
                .help("TLS certificate filename"),
        )
        .arg(
            Arg::new("http.tls.key")
                .long("tls-key")
                .action(ArgAction::Set)
                .value_name("FILENAME")
                .help("TLS key filename"),
        )
        .arg(
            Arg::new("input.filename")
                .long("input")
                .action(ArgAction::Set)
                .value_name("FILENAME")
                .help("Input Eve file to read"),
        )
        .arg(
            Arg::new("end")
                .action(ArgAction::SetTrue)
                .long("end")
                .help("Read from end (tail) of input file"),
        )
        .arg(
            Arg::new("sqlite")
                .action(ArgAction::SetTrue)
                .long("sqlite")
                .help("Use SQLite"),
        )
        .arg(
            Arg::new("geoip.disabled")
                .action(ArgAction::SetTrue)
                .long("disable-geoip")
                .help("Disable GeoIP"),
        )
        .arg(Arg::new("input-start").long("input-start").hide(true));

    let oneshot = Command::new("oneshot")
        .about("Import a single eve.json and review in EveBox")
        .arg(
            // This is here just to hide -D from oneshot mode.
            Arg::new("data-directory")
                .long("data-directory")
                .short('D')
                .action(ArgAction::Set)
                .value_name("DIR")
                .help("Data directory")
                .hide(true),
        )
        .arg(
            Arg::new("limit")
                .long("limit")
                .action(ArgAction::Set)
                .help("Limit the number of events read"),
        )
        .arg(
            Arg::new("no-open")
                .long("no-open")
                .action(ArgAction::SetTrue)
                .help("Don't open browser"),
        )
        .arg(
            Arg::new("no-wait")
                .long("no-wait")
                .action(ArgAction::SetTrue)
                .help("Don't wait for events to load"),
        )
        .arg(
            Arg::new("database-filename")
                .long("database-filename")
                .action(ArgAction::Set)
                .default_value("./oneshot.sqlite")
                .value_name("FILENAME")
                .help("Database filename"),
        )
        // --host, but keep th name as http.host to be campatible with the
        // EVEBOX_HTTP_HOST environment variable.
        .arg(
            clap::Arg::new("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .action(ArgAction::Set)
                .default_value("127.0.0.1")
                .help("Hostname/IP address to bind to"),
        )
        .arg(Arg::new("INPUT").required(true).index(1));

    let mut parser = parser
        .subcommand(server)
        .subcommand(oneshot)
        .subcommand(evebox::commands::agent::command())
        .subcommand(evebox::commands::config::config_subcommand())
        .subcommand(evebox::commands::print::command())
        .subcommand(evebox::commands::elastic::main::main_options())
        .subcommand(evebox::commands::sqlite::command());
    let matches = parser.clone().get_matches();

    // Initialize logging.
    let log_level = {
        let verbosity = matches.get_count("verbose");
        if verbosity > 1 {
            tracing::Level::TRACE
        } else if verbosity > 0 {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    };
    logger::init_logger(log_level)?;
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
        Some(("oneshot", args)) => evebox::commands::oneshot::main(args).await,
        Some(("agent", args)) => evebox::commands::agent::main(args).await,
        Some(("config", args)) => evebox::commands::config::main(args),
        Some(("print", args)) => evebox::commands::print::main(args),
        Some(("elastic", args)) => evebox::commands::elastic::main::main(args).await,
        Some(("sqlite", args)) => evebox::commands::sqlite::main(args).await,
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
