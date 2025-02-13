// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use clap::{value_parser, ArgAction};
use clap::{Arg, Command};
use evebox::logger;
use evebox::version;
use tracing::error;

/// Something like the latest Cargo styling.
fn get_clap_style() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(clap::builder::styling::AnsiColor::Green.on_default())
        .usage(clap::builder::styling::AnsiColor::Green.on_default())
        .literal(clap::builder::styling::AnsiColor::Cyan.on_default())
        .placeholder(clap::builder::styling::AnsiColor::Cyan.on_default())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .styles(get_clap_style())
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
                .hide_env(true)
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
            Arg::new("config-directory")
                .long("config-directory")
                .short('C')
                .action(ArgAction::Set)
                .value_name("DIR")
                .help("Configuration directory")
                .env("EVEBOX_CONFIG_DIRECTORY")
                .hide_env(true)
                .hide(true)
                .global(true),
        )
        .arg(
            clap::Arg::new("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .action(ArgAction::Set)
                .default_value("127.0.0.1")
                .env("EVEBOX_HTTP_HOST")
                .hide_env(true)
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
                .hide_env(true)
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
                .hide_env(true)
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
            clap::Arg::new("database.elasticsearch.cacert")
                .long("elasticsearch-cacert")
                .action(ArgAction::Set)
                .value_name("FILENAME")
                .help("Elasticsearch CA certificate filename")
                .env("EVEBOX_ELASTICSEARCH_CACERT"),
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
                .hide_env(true)
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
            Arg::new("authentication.required")
                .action(ArgAction::SetFalse)
                .long("no-auth")
                .help("Disable authentication")
                .alias("no-authentication")
                .alias("disable-authentication"),
        )
        .arg(
            Arg::new("http.tls.enabled")
                .action(ArgAction::SetFalse)
                .long("no-tls")
                .help("Disable TLS")
                .env("EVEBOX_HTTP_TLS_ENABLED")
                .hide_env(true),
        )
        .arg(
            // We accept, but ignore --tls as TLS is now enabled by
            // default and can be turned off with --no-tls.
            Arg::new("http.tls.__ignore_enabled")
                .long("tls")
                .action(ArgAction::SetTrue)
                .hide(true),
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
        .arg(
            Arg::new("input.paths")
                .value_name("EVE")
                .num_args(0..)
                .help("One or more Suricata EVE/JSON files"),
        );

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
        .subcommand(evebox::cli::agent::command())
        .subcommand(evebox::cli::config::config_subcommand())
        .subcommand(evebox::cli::print::command())
        .subcommand(evebox::cli::elastic::main::main_options())
        .subcommand(evebox::cli::sqlite::command())
        .subcommand(evebox::cli::update::args());
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
        Some(("oneshot", args)) => evebox::cli::oneshot::main(args).await,
        Some(("agent", args)) => evebox::cli::agent::main(args).await,
        Some(("config", args)) => evebox::cli::config::main(args).await,
        Some(("print", args)) => evebox::cli::print::main(args),
        Some(("elastic", args)) => evebox::cli::elastic::main::main(args).await,
        Some(("sqlite", args)) => evebox::cli::sqlite::main(args).await,
        Some(("update", args)) => evebox::cli::update::main(args).await,
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
