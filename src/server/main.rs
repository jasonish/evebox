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

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use warp::filters::BoxedFilter;
use warp::{self, Filter, Future};

use crate::bookmark;
use crate::datastore::Datastore;
use crate::elastic;
use crate::eve::filters::EveBoxMetadataFilter;
use crate::eve::processor::Processor;
use crate::eve::EveReader;
use crate::logger::log;
use crate::server::session::Session;
use crate::server::AuthenticationType;
use crate::settings::Settings;
use crate::sqlite;
use crate::sqlite::configrepo::ConfigRepo;

use super::{ServerConfig, ServerContext};

pub async fn main(args: &clap::ArgMatches<'static>) -> Result<()> {
    crate::version::log_version();

    let mut settings = Settings::new(args);
    let mut config = ServerConfig::default();
    config.port = settings.get("http.port")?;
    config.host = settings.get("http.host")?;
    config.tls_enabled = settings.get_bool("http.tls.enabled")?;
    config.tls_cert_filename = settings.get_or_none("http.tls.certificate")?;
    config.tls_key_filename = settings.get_or_none("http.tls.key")?;
    config.datastore = settings.get("database.type")?;
    config.elastic_url = settings.get("database.elasticsearch.url")?;
    config.elastic_index = settings.get("database.elasticsearch.index")?;
    config.elastic_username = settings.get_or_none("database.elasticsearch.username")?;
    config.elastic_password = settings.get_or_none("database.elasticsearch.password")?;
    config.data_directory = settings.get_or_none("data-directory")?;
    config.database_retention_period = settings.get_or_none("database.retention-period")?;
    if let Ok(val) = settings.get_bool("database.elasticsearch.disable-certificate-check") {
        config.no_check_certificate = val;
    } else {
        config.no_check_certificate = settings.get_bool("no-check-certificate")?;
    }

    config.authentication_required = settings.get_bool("authentication.required")?;
    if config.authentication_required {
        log::info!("Authentication is required...");
        if let Some(auth_type) = settings.get_or_none::<String>("authentication.type")? {
            config.authentication_type = match auth_type.as_ref() {
                "username" => AuthenticationType::Username,
                "usernamepassword" => AuthenticationType::UsernamePassword,
                _ => {
                    return Err(anyhow!("Bad authentication type: {}", auth_type));
                }
            };
        }
    }

    // Do we need a data-directory? If so, make sure its set.
    let data_directory_required = if config.datastore == "sqlite" {
        true
    } else {
        false
    };

    if data_directory_required && config.data_directory.is_none() {
        log::error!("A data-directory is required");
        std::process::exit(1);
    }

    // Command line only.
    config.http_log = args.occurrences_of("access-log") > 0;

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let context = build_context(config.clone(), None).await?;

    let input_filename: Option<String> = settings.get_or_none("input.filename")?;
    if let Some(input_filename) = &input_filename {
        let end = settings.get_bool("end")?;

        let bookmark_directory: Option<String> =
            settings.get_or_none("input.bookmark-directory")?;
        let bookmark_filename = get_bookmark_filename(
            input_filename,
            bookmark_directory.as_deref(),
            config.data_directory.as_deref(),
        );
        log::info!(
            "Using bookmark filename {:?} for input {:?}",
            bookmark_filename,
            input_filename
        );

        let importer = if let Some(importer) = context.datastore.get_importer() {
            importer
        } else {
            log::error!("No importer implementation for this database.");
            std::process::exit(1);
        };
        let mut filters = Vec::new();
        filters.push(
            EveBoxMetadataFilter {
                filename: Some(input_filename.clone()),
            }
            .into(),
        );
        let reader = EveReader::new(input_filename);
        let mut processor = Processor::new(reader, importer.clone());
        processor.report_interval = Duration::from_secs(60);
        processor.filters = filters;
        processor.end = end;
        processor.bookmark_filename = bookmark_filename;
        log::info!("Starting reader for {}", input_filename);
        tokio::spawn(async move {
            processor.run().await;
        });
    }

    let context = Arc::new(context);
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let server = build_server(&config, context.clone());

    log::info!(
        "Starting server on {}:{}, tls={}",
        config.host,
        config.port,
        config.tls_enabled
    );
    if config.tls_enabled {
        let cert_path = if let Some(filename) = config.tls_cert_filename {
            filename
        } else {
            log::error!("TLS request but not certificate filename provided");
            std::process::exit(1);
        };
        let key_path = if let Some(filename) = config.tls_key_filename {
            filename
        } else {
            log::error!("TLS requested but not key filename provided");
            std::process::exit(1);
        };
        server
            .tls()
            .cert_path(cert_path)
            .key_path(key_path)
            .run(addr)
            .await;
    } else {
        match server.try_bind_ephemeral(addr) {
            Err(err) => {
                log::error!("Failed to start server: {}", err);
                std::process::exit(1);
            }
            Ok((_, bound)) => {
                bound.await;
            }
        }
    }
    Ok(())
}

pub async fn build_server_try_bind(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> Result<
    impl Future<Output = ()> + Send + Sync + 'static,
    Box<dyn std::error::Error + Sync + Send>,
> {
    let server = build_server(config, context);
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let server = server.try_bind_ephemeral(addr)?.1;
    Ok(server)
}

pub fn build_server(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> warp::Server<BoxedFilter<(impl warp::Reply,)>> {
    let session_filter = build_session_filter(context.clone()).boxed();
    let routes = super::filters::api_routes(context, session_filter)
        .or(resource_filters())
        .recover(super::rejection::rejection_handler);
    let mut headers = warp::http::header::HeaderMap::new();
    headers.insert(
        "X-EveBox-Git-Revision",
        warp::http::header::HeaderValue::from_static(crate::version::build_rev()),
    );

    let routes = routes.with(warp::reply::with::headers(headers));
    let do_log_http = config.http_log;
    let http_log = warp::log::custom(move |info| {
        if do_log_http {
            log::info!("{:?}", info.remote_addr());
        }
    });
    let routes = routes.with(http_log);
    warp::serve(routes.boxed())
}

pub async fn build_context(
    config: ServerConfig,
    datastore: Option<Datastore>,
) -> anyhow::Result<ServerContext> {
    let config_repo = if let Some(directory) = &config.data_directory {
        let filename = PathBuf::from(directory).join("config.sqlite");
        log::info!("Configuration database filename: {:?}", filename);
        ConfigRepo::new(Some(&filename))?
    } else {
        log::info!("Using temporary in-memory configuration database");
        ConfigRepo::new(None)?
    };
    let mut context = ServerContext::new(config, Arc::new(config_repo));
    if let Some(datastore) = datastore {
        context.datastore = datastore;
    } else {
        configure_datastore(&mut context).await?;
    }
    Ok(context)
}

async fn configure_datastore(context: &mut ServerContext) -> anyhow::Result<()> {
    let config = &context.config;
    match config.datastore.as_ref() {
        "elasticsearch" => {
            let mut client = elastic::ClientBuilder::new(&config.elastic_url);
            if let Some(username) = &config.elastic_username {
                client.with_username(username);
            }
            if let Some(password) = &config.elastic_password {
                client.with_password(password);
            }
            client.disable_certificate_validation(config.no_check_certificate);

            let client = client.build();

            match client.get_version().await {
                Err(err) => {
                    log::error!(
                        "Failed to get Elasticsearch version, things may not work right: error={}",
                        err
                    );
                }
                Ok(version) => {
                    if version.major < 6 {
                        return Err(anyhow!(
                            "Elasticsearch versions less than 6 are not supported"
                        ));
                    }
                    log::info!("Found Elasticsearch version {}", version.version);
                }
            }

            let eventstore = elastic::EventStore {
                base_index: config.elastic_index.clone(),
                index_pattern: format!("{}-*", config.elastic_index),
                client: client,
            };
            context.features.reporting = true;
            context.features.comments = true;
            context.elastic = Some(eventstore.clone());
            context.datastore = Datastore::Elastic(eventstore.clone());
        }
        "sqlite" => {
            let db_filename = if let Some(dir) = &config.data_directory {
                std::path::PathBuf::from(dir).join("events.sqlite")
            } else if let Some(filename) = &config.sqlite_filename {
                std::path::PathBuf::from(filename)
            } else {
                panic!("data-directory required");
            };
            let connection_builder = sqlite::ConnectionBuilder {
                filename: Some(db_filename),
            };
            let mut connection = connection_builder.open().unwrap();
            sqlite::init_event_db(&mut connection).unwrap();
            let connection = Arc::new(Mutex::new(connection));

            let eventstore = sqlite::eventstore::SQLiteEventStore {
                connection: connection.clone(),
                importer: sqlite::importer::Importer::new(connection.clone()),
            };
            context.datastore = Datastore::SQLite(eventstore);

            // Setup retention job.
            if let Some(period) = config.database_retention_period {
                if period > 0 {
                    log::info!("Setting data retention period to {} days", period);
                    let retention_config = sqlite::retention::RetentionConfig { days: period };
                    let connection = connection.clone();
                    tokio::task::spawn_blocking(|| {
                        sqlite::retention::retention_task(retention_config, connection);
                    });
                }
            }
        }
        _ => panic!("unsupported datastore"),
    }
    Ok(())
}

pub fn resource_filters(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let index = warp::get()
        .and(warp::path::end())
        .map(|| super::asset::new_static_or_404("index.html"));
    let favicon = warp::get()
        .and(warp::path("favicon.ico"))
        .and(warp::path::end())
        .map(|| super::asset::new_static_or_404("favicon.ico"));
    let public = warp::get()
        .and(warp::path("public"))
        .and(warp::path::tail())
        .map(|path: warp::filters::path::Tail| super::asset::new_static_or_404(path.as_str()));
    return index.or(favicon).or(public);
}

#[derive(Debug)]
pub enum GenericError {
    NotFound,
    AuthenticationRequired,
}

impl warp::reject::Reject for GenericError {}

impl From<GenericError> for warp::Rejection {
    fn from(err: GenericError) -> Self {
        warp::reject::custom(err)
    }
}

pub fn build_session_filter(
    context: Arc<ServerContext>,
) -> impl Filter<Extract = (Arc<Session>,), Error = warp::Rejection> + Clone {
    let context = warp::any().map(move || context.clone());

    let session_id = warp::header("x-evebox-session-id")
        .map(|session_id: String| Some(session_id))
        .or(warp::any().map(|| None))
        .unify();

    let remote_user = warp::header("REMOTE_USER")
        .map(|remote_user: String| Some(remote_user))
        .or(warp::any().map(|| None))
        .unify();

    warp::any()
        .and(session_id)
        .and(context)
        .and(warp::filters::addr::remote())
        .and(warp::filters::path::full())
        .and(remote_user)
        .and_then(
            move |session_id: Option<String>,
                  context: Arc<ServerContext>,
                  addr,
                  _path,
                  remote_user: Option<String>| async move {
                if let Some(session_id) = session_id {
                    let session = context.session_store.get(&session_id);
                    if let Some(session) = session {
                        return Ok(session);
                    }
                }

                match context.config.authentication_type {
                    AuthenticationType::Anonymous => {
                        let username = if let Some(username) = remote_user {
                            username
                        } else {
                            "<anonymous>".to_string()
                        };
                        log::info!(
                            "Creating anonymous session for user from {:?} with name {}",
                            addr,
                            username
                        );
                        let mut session = Session::new();
                        session.username = Some(username);
                        let session = Arc::new(session);
                        context.session_store.put(session.clone()).unwrap();
                        Ok::<_, warp::Rejection>(session)
                    }
                    _ => Err::<_, warp::Rejection>(warp::reject::custom(
                        GenericError::AuthenticationRequired,
                    )),
                }
            },
        )
}

fn get_bookmark_filename(
    input_filename: &str,
    input_bookmark_dir: Option<&str>,
    data_directory: Option<&str>,
) -> Option<PathBuf> {
    // First priority is the input_bookmark_directory.
    if let Some(directory) = input_bookmark_dir {
        return Some(bookmark::bookmark_filename(input_filename, directory));
    }

    // Otherwise see if there is a file with the same name as the input filename but
    // suffixed with ".bookmark".
    let legacy_filename = format!("{}.bookmark", input_filename);
    if let Ok(_meta) = std::fs::metadata(&legacy_filename) {
        log::warn!(
            "Found legacy bookmark file, checking if writable: {}",
            &legacy_filename
        );
        match test_writable(&legacy_filename) {
            Ok(_) => {
                log::warn!("Using legacy bookmark filename: {}", &legacy_filename);
                return Some(PathBuf::from(&legacy_filename));
            }
            Err(err) => {
                log::error!(
                    "Legacy bookmark filename not writable, will not use: filename={}, error={}",
                    legacy_filename,
                    err
                );
            }
        }
    }

    // Do we have a global data-directory, and is it writable?
    if let Some(directory) = data_directory {
        let bookmark_filename = bookmark::bookmark_filename(input_filename, directory);
        log::debug!("Checking {:?} for writability", &bookmark_filename);
        if let Err(err) = test_writable(&bookmark_filename) {
            log::error!("{:?} not writable: {}", &bookmark_filename, err);
        } else {
            return Some(bookmark_filename);
        }
    }

    // All that failed, check the current directory.
    let bookmark_filename = bookmark::bookmark_filename(input_filename, ".");
    if test_writable(&bookmark_filename).is_ok() {
        return Some(bookmark_filename);
    }

    None
}

fn test_writable<T: AsRef<Path>>(filename: T) -> anyhow::Result<()> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(filename)?;
    Ok(())
}
