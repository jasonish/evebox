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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync;

use crate::eve;
use crate::geoip;
use crate::logger::log;
use crate::settings::Settings;
use crate::sqlite;

pub async fn main(args: &clap::ArgMatches<'static>) -> anyhow::Result<()> {
    let mut settings = Settings::new(args);
    let limit: u64 = settings.get("limit").unwrap_or(0);
    let no_open: bool = settings.get_bool("no-open")?;
    let no_wait: bool = settings.get_bool("no-wait")?;
    let db_filename: String = settings.get("database-filename")?;
    let input = args.value_of("INPUT").unwrap().to_string();
    let host: String = settings.get("http.host")?;

    log::info!("Using database filename {}", &db_filename);

    let mut db = sqlite::ConnectionBuilder::filename(Some(&PathBuf::from(&db_filename))).open()?;
    sqlite::init_event_db(&mut db)?;
    let db = Arc::new(Mutex::new(db));

    let import_task = {
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(err) = run_import(db, limit, &input).await {
                log::error!("Import failure: {}", err);
            }
        })
    };

    if !no_wait {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Got CTRL-C, will start server now. Hit CTRL-C again to exit");
            }
            _ = import_task => {}
        }
    }

    let (port_tx, mut port_rx) = sync::mpsc::unbounded_channel::<u16>();

    let server = {
        let db_filename = db_filename.clone();
        let host = host.clone();
        tokio::spawn(async move {
            let mut port = 5636;
            loop {
                let sqlite_datastore = sqlite::eventstore::SQLiteEventStore::new(db.clone());
                let ds = crate::datastore::Datastore::SQLite(sqlite_datastore);
                let config = crate::server::ServerConfig {
                    port: port.to_string(),
                    host: host.clone(),
                    elastic_url: "".to_string(),
                    elastic_index: "".to_string(),
                    no_check_certificate: false,
                    datastore: "sqlite".to_string(),
                    sqlite_filename: Some(db_filename.clone()),
                    http_log: false,
                    ..crate::server::ServerConfig::default()
                };
                let context = match crate::server::build_context(config.clone(), Some(ds)).await {
                    Ok(context) => Arc::new(context),
                    Err(err) => {
                        log::error!("Failed to build server context: {}", err);
                        std::process::exit(1);
                    }
                };
                log::debug!("Successfully build server context");
                match crate::server::build_server_try_bind(&config, context).await {
                    Ok(server) => {
                        log::debug!("Looks like a successful bind to port {}", port);
                        port_tx.send(port).unwrap();
                        server.await;
                        break;
                    }
                    Err(_) => {
                        log::warn!(
                            "Failed to start server on port {}, will try {}",
                            port,
                            port + 1
                        );
                        port += 1;
                    }
                }
            }
        })
    };

    let port = port_rx.recv().await.unwrap();
    let url = format!("http://{}:{}", host, port);
    log::info!("Server started at {}", url);

    let connect_url = if host == "0.0.0.0" {
        format!("http://127.0.0.1:{}", port)
    } else {
        format!("http://{}:{}", host, port)
    };

    if !no_open {
        if let Err(err) = webbrowser::open(&connect_url) {
            log::error!("Failed to open {} in browser: {}", url, err);
        }

        log::info!(
            "If your browser didn't open, try connecting to {}",
            connect_url
        );
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        log::info!("Got CTRL-C, exiting");
        let _ = std::fs::remove_file(&db_filename);
        std::process::exit(0);
    });

    server.await?;
    Ok(())
}

async fn run_import(
    db: Arc<Mutex<rusqlite::Connection>>,
    limit: u64,
    input: &str,
) -> anyhow::Result<()> {
    let geoipdb = match geoip::GeoIP::open(None) {
        Ok(geoipdb) => Some(geoipdb),
        Err(_) => None,
    };
    let mut indexer = sqlite::importer::Importer::new(db);
    let mut reader = eve::reader::EveReader::new(&input);
    log::info!("Reading {} ({} bytes)", input, reader.file_size());
    let mut last_percent = 0;
    let mut count = 0;
    loop {
        match reader.next_record() {
            Ok(None) | Err(_) => {
                break;
            }
            Ok(Some(mut next)) => {
                if let Some(geoipdb) = &geoipdb {
                    geoipdb.add_geoip_to_eve(&mut next);
                }
                indexer.submit(next).await?;
                count += 1;
                let size = reader.file_size();
                let offset = reader.offset();
                let pct = ((offset as f64 / size as f64) * 100.0) as u64;
                if pct != last_percent {
                    log::info!("{}: {} events ({}%)", input, count, pct);
                    last_percent = pct;
                }
                if indexer.pending() > 300 {
                    indexer.commit().await?;
                }
            }
        }
        if limit > 0 && count == limit {
            break;
        }
    }
    indexer.commit().await?;
    log::info!("Read {} events", count);
    Ok(())
}
