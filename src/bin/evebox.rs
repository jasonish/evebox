// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![allow(clippy::redundant_field_names)]

use clap::{Arg, SubCommand};
use evebox::logger::{self, log};
use evebox::version;

#[tokio::main]
async fn main() {
    if let Err(err) = _main().await {
        log::error!("{}", err);
        std::process::exit(1);
    }
}

async fn _main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = clap::App::new("EveBox")
        .version(std::env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .multiple(true)
                .global(true)
                .help("Increase verbosity"),
        )
        .arg(
            Arg::with_name("data-directory")
                .long("data-directory")
                .short("D")
                .takes_value(true)
                .value_name("DIR")
                .help("Data directory")
                .global(true),
        )
        .subcommand(clap::SubCommand::with_name("version").about("Display version"));

    let sqlite_import = SubCommand::with_name("sqlite-import")
        .about("Import to SQLite")
        .arg(
            Arg::with_name("oneshot")
                .long("oneshot")
                .help("One shot mode (exit on EOF"),
        )
        .arg(
            Arg::with_name("end")
                .long("end")
                .help("Start at end of file"),
        )
        .arg(Arg::with_name("INPUT").required(true).index(1));

    let server = clap::SubCommand::with_name("server")
        .about("EveBox Server")
        .arg(
            clap::Arg::with_name("config")
                .long("config")
                .short("c")
                .takes_value(true)
                .value_name("FILE")
                .help("Configuration filename"),
        )
        .arg(
            clap::Arg::with_name("http.host")
                .long("host")
                .value_name("HOSTNAME")
                .takes_value(true)
                .default_value("127.0.0.1")
                .help("Hostname/IP address to bind to"),
        )
        .arg(
            clap::Arg::with_name("http.port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .takes_value(true)
                .default_value("5636")
                .help("Port to bind to"),
        )
        .arg(
            clap::Arg::with_name("database.elasticsearch.url")
                .short("e")
                .long("elasticsearch")
                .takes_value(true)
                .value_name("URL")
                .default_value("http://localhost:9200")
                .help("Elastic Search URL"),
        )
        .arg(
            clap::Arg::with_name("database.elasticsearch.index")
                .short("i")
                .long("index")
                .takes_value(true)
                .default_value("logstash")
                .value_name("INDEX")
                .help("Elastic Search index prefix"),
        )
        .arg(
            Arg::with_name("database.type")
                .long("database")
                .aliases(&["datastore"])
                .takes_value(true)
                .value_name("DATABASE")
                .default_value("elasticsearch")
                .help("Database type"),
        )
        .arg(
            Arg::with_name("sqlite-filename")
                .long("sqlite-filename")
                .takes_value(true)
                .value_name("FILENAME")
                .default_value("events.sqlite")
                .help("SQLite events filename"),
        )
        .arg(
            clap::Arg::with_name("no-check-certificate")
                .short("k")
                .long("no-check-certificate")
                .help("Disable TLS certificate validation"),
        )
        .arg(
            Arg::with_name("access-log")
                .long("access-log")
                .help("Enable HTTP access logging"),
        )
        .arg(
            Arg::with_name("http.tls.enabled")
                .long("tls")
                .help("Enable TLS"),
        )
        .arg(
            Arg::with_name("http.tls.certificate")
                .long("tls-cert")
                .takes_value(true)
                .value_name("FILENAME")
                .help("TLS certificate filename"),
        )
        .arg(
            Arg::with_name("http.tls.key")
                .long("tls-key")
                .takes_value(true)
                .value_name("FILENAME")
                .help("TLS key filename"),
        )
        .arg(
            Arg::with_name("input.filename")
                .long("input")
                .takes_value(true)
                .value_name("FILENAME")
                .help("Input Eve file to read"),
        )
        .arg(
            Arg::with_name("end")
                .long("end")
                .help("Read from end (tail) of input file"),
        )
        .arg(
            Arg::with_name("input-start")
                .long("input-start")
                .hidden(true),
        );

    let agent = SubCommand::with_name("agent")
        .about("EveBox Agent")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .takes_value(true)
                .help("Configuration file"),
        )
        .arg(
            Arg::with_name("server.url")
                .long("server")
                .takes_value(true)
                .value_name("URL")
                .help("EveBox Server URL"),
        )
        .arg(Arg::with_name("geoip.enabled").long("enable-geoip"))
        .arg(
            Arg::with_name("stdout")
                .long("stdout")
                .help("Print events to stdout"),
        );

    let oneshot = SubCommand::with_name("oneshot")
        .about("Import a single eve.json and review in EveBox")
        .arg(
            Arg::with_name("limit")
                .long("limit")
                .takes_value(true)
                .help("Limit the number of events read"),
        )
        .arg(
            Arg::with_name("no-open")
                .long("no-open")
                .help("Don't open browser"),
        )
        .arg(
            Arg::with_name("no-wait")
                .long("no-wait")
                .help("Don't wait for events to load"),
        )
        .arg(
            Arg::with_name("database-filename")
                .long("database-filename")
                .takes_value(true)
                .default_value("./oneshot.sqlite")
                .help("Database filename"),
        )
        .arg(Arg::with_name("INPUT").required(true).index(1));

    let elastic_import = SubCommand::with_name("elastic-import")
        .alias("esimport")
        .about("Import to Elastic Search")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .takes_value(true)
                .help("Configuration file"),
        )
        .arg(
            Arg::with_name("oneshot")
                .long("oneshot")
                .help("One shot mode (exit on EOF)"),
        )
        .arg(
            Arg::with_name("end")
                .long("end")
                .help("Start at end of file"),
        )
        .arg(
            clap::Arg::with_name("elasticsearch")
                .short("e")
                .long("elasticsearch")
                .takes_value(true)
                .default_value("http://localhost:9200")
                .help("Elastic Search URL"),
        )
        .arg(
            Arg::with_name("index")
                .long("index")
                .takes_value(true)
                .default_value("logstash")
                .help("Elastic Search index prefix"),
        )
        .arg(
            Arg::with_name("bookmark")
                .long("bookmark")
                .help("Enable bookmarking"),
        )
        .arg(
            Arg::with_name("bookmark-filename")
                .long("bookmark-filename")
                .takes_value(true)
                //.default_value(".bookmark")
                .default_value("")
                .help("Bookmark filename"),
        )
        .arg(
            Arg::with_name("bookmark-dir")
                .long("bookmark-dir")
                .takes_value(true)
                .default_value(".")
                .help("Bookmark directory"),
        )
        .arg(
            Arg::with_name("stdout")
                .long("stdout")
                .help("Print events to stdout"),
        )
        .arg(
            Arg::with_name("username")
                .long("username")
                .short("u")
                .takes_value(true)
                .help("Elasticsearch username"),
        )
        .arg(
            Arg::with_name("password")
                .long("password")
                .short("p")
                .takes_value(true)
                .help("Elasticsearch password"),
        )
        .arg(
            clap::Arg::with_name(evebox::commands::elastic_import::NO_CHECK_CERTIFICATE)
                .short("k")
                .long("no-check-certificate")
                .help("Disable TLS certificate validation"),
        )
        .arg(
            Arg::with_name("geoip.disable")
                .long("no-geoip")
                .help("Disable GeoIP"),
        )
        .arg(
            Arg::with_name("geoip.database-filename")
                .long("geoip-database")
                .takes_value(true)
                .value_name("filename")
                .help("GeoIP database filename"),
        )
        .arg(
            Arg::with_name("input")
                .multiple(true)
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
        ("server", Some(args)) => {
            evebox::server::main(args).await?;
            Ok(())
        }
        ("version", _) => {
            version::print_version();
            Ok(())
        }
        ("sqlite-import", Some(args)) => evebox::commands::sqlite_import::main(args).await,
        ("elastic-import", Some(args)) => {
            if let Err(err) = evebox::commands::elastic_import::main(&args).await {
                log::error!("{}", err);
                std::process::exit(1);
            }
            Ok(())
        }
        ("elastic-debug", Some(args)) => evebox::commands::elastic_debug::main(args).await,
        ("oneshot", Some(args)) => evebox::commands::oneshot::main(&args).await,
        ("agent", Some(args)) => evebox::agent::main(&args).await,
        ("config", Some(args)) => evebox::commands::config::main(&args),
        ("", None) => {
            parser.print_help().ok();
            // print_help doesn't output a new line at the end, so fix that up...
            println!();
            std::process::exit(1);
        }
        (command, _) => {
            log::error!("command \"{}\" not implemented yet", command);
            std::process::exit(1);
        }
    };
    if let Err(err) = rc {
        log::error!("error: {}", err);
        std::process::exit(1);
    } else {
        Ok(())
    }
}

fn elastic_debug() -> clap::App<'static, 'static> {
    SubCommand::with_name("elastic-debug").arg(
        Arg::with_name("elasticsearch")
            .long("elasticsearch")
            .short("e")
            .takes_value(true)
            .default_value("127.0.0.1"),
    )
}
