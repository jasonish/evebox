// Copyright 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

#![allow(clippy::redundant_field_names)]

use clap::{Arg, Command};
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
                .help("Hostname/IP address to bind to"),
        )
        .arg(
            clap::Arg::new("http.port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .takes_value(true)
                .default_value("5636")
                .help("Port to bind to"),
        )
        .arg(
            clap::Arg::new("database.elasticsearch.url")
                .short('e')
                .long("elasticsearch")
                .takes_value(true)
                .value_name("URL")
                .default_value("http://localhost:9200")
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
                .help("Enable HTTP access logging"),
        )
        .arg(Arg::new("http.tls.enabled").long("tls").help("Enable TLS"))
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

    let agent = Command::new("agent")
        .about("EveBox Agent")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .takes_value(true)
                .help("Configuration file"),
        )
        .arg(
            Arg::new("server.url")
                .long("server")
                .takes_value(true)
                .value_name("URL")
                .help("EveBox Server URL"),
        )
        .arg(Arg::new("geoip.enabled").long("enable-geoip"))
        .arg(
            Arg::new("stdout")
                .long("stdout")
                .help("Print events to stdout"),
        )
        .arg(
            Arg::new("bookmark-directory")
                .long("bookmark-directory")
                .takes_value(true)
                .hide(true),
        );

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
        .alias("esimport")
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
                .help("Elastic Search URL"),
        )
        .arg(
            Arg::new("index")
                .long("index")
                .takes_value(true)
                .default_value("logstash")
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
                .help("Bookmark filename"),
        )
        .arg(
            Arg::new("bookmark-dir")
                .long("bookmark-dir")
                .takes_value(true)
                .default_value(".")
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
        .subcommand(agent)
        .subcommand(sqlite_import)
        .subcommand(evebox::commands::config::config_subcommand())
        .subcommand(elastic_debug());

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
        Some(("elastic-debug", args)) => evebox::commands::elastic_debug::main(args).await,
        Some(("oneshot", args)) => evebox::commands::oneshot::main(args).await,
        Some(("agent", args)) => evebox::agent::main(args).await,
        Some(("config", args)) => evebox::commands::config::main(args),
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

fn elastic_debug() -> clap::Command<'static> {
    Command::new("elastic-debug").arg(
        Arg::new("elasticsearch")
            .long("elasticsearch")
            .short('e')
            .takes_value(true)
            .default_value("127.0.0.1"),
    )
}
