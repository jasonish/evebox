// Copyright (C) 2020 Jason Ish
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

use crate::prelude::*;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use axum::async_trait;
use axum::body::Body;
use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::extract::{ConnectInfo, Extension, FromRequest, RequestParts};
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::{AddExtensionLayer, Router, Server};
use hyper::server::conn::AddrIncoming;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse};
use tracing::Level;

use crate::bookmark;
use crate::datastore::Datastore;
use crate::elastic;
use crate::eve::filters::{AddRuleFilter, EveBoxMetadataFilter};
use crate::eve::processor::Processor;
use crate::eve::EveReader;
use crate::server::session::Session;
use crate::server::{api, AuthenticationType};
use crate::settings::Settings;
use crate::sqlite;
use crate::sqlite::configrepo::ConfigRepo;

use super::{ServerConfig, ServerContext};
use crate::elastic::Client;

fn load_event_services(filename: &str) -> anyhow::Result<serde_json::Value> {
    let finput = std::fs::File::open(filename)?;
    let yaml_value: serde_yaml::Value = serde_yaml::from_reader(finput)?;
    let json_value = serde_json::to_value(&yaml_value["event-services"])?;
    Ok(json_value)
}

pub async fn main(args: &clap::ArgMatches<'static>) -> Result<()> {
    crate::version::log_version();

    let config_filename = args.value_of("config");

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
    config.elastic_no_index_suffix = settings.get_bool("database.elasticsearch.no-index-suffix")?;
    config.elastic_ecs = settings.get_bool("database.elasticsearch.ecs")?;
    config.elastic_username = settings.get_or_none("database.elasticsearch.username")?;
    config.elastic_password = settings.get_or_none("database.elasticsearch.password")?;
    config.data_directory = settings.get_or_none("data-directory")?;
    config.database_retention_period = settings.get_or_none("database.retention-period")?;
    if let Ok(val) = settings.get_bool("database.elasticsearch.disable-certificate-check") {
        if val {
            config.no_check_certificate = true;
        } else {
            config.no_check_certificate = settings.get_bool("no-check-certificate")?;
        }
    }
    config.http_request_logging = settings.get_bool("http.request-logging")?;
    config.http_reverse_proxy = settings.get_bool("http.reverse-proxy")?;

    debug!(
        "Certificate checks disabled: {}",
        config.no_check_certificate,
    );

    config.authentication_required = settings.get_bool("authentication.required")?;
    if config.authentication_required {
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
    let data_directory_required = config.datastore == "sqlite";

    if data_directory_required && config.data_directory.is_none() {
        error!("A data-directory is required");
        std::process::exit(1);
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let mut context = build_context(config.clone(), None).await?;

    if let Some(filename) = config_filename {
        match load_event_services(filename) {
            Err(err) => {
                error!("Failed to load event-services: {}", err);
            }
            Ok(event_services) => {
                context.event_services = Some(event_services);
            }
        }
    }

    let input_enabled = {
        if settings.args.occurrences_of("input.filename") > 0 {
            true
        } else {
            settings.get_bool("input.enabled")?
        }
    };

    // This needs some cleanup. We load the input file names here, but configure
    // it later down.  Also, the filters (rules) are unlikely required if we
    // don't have an input enabled.
    let input_filenames = if input_enabled {
        let input_filename: Option<String> = settings.get_or_none("input.filename")?;
        let mut input_filenames = Vec::new();
        if let Some(input_filename) = &input_filename {
            for path in crate::path::expand(input_filename)? {
                let path = path.display().to_string();
                input_filenames.push(path);
            }
        }
        input_filenames
    } else {
        Vec::new()
    };

    let mut shared_filters = Vec::new();

    match settings.get::<Vec<String>>("input.rules") {
        Ok(rules) => {
            let rulemap = crate::rules::load_rules(&rules);
            let rulemap = Arc::new(rulemap);
            shared_filters.push(crate::eve::filters::EveFilter::AddRuleFilter(
                AddRuleFilter {
                    map: rulemap.clone(),
                },
            ));
            crate::rules::watch_rules(rulemap);
        }
        Err(err) => match err {
            config::ConfigError::NotFound(_) => {}
            _ => {
                error!("Failed to read input.rules configuration: {}", err);
            }
        },
    }

    let shared_filters = Arc::new(shared_filters);

    for input_filename in &input_filenames {
        let end = settings.get_bool("end")?;
        let bookmark_directory: Option<String> =
            settings.get_or_none("input.bookmark-directory")?;
        let bookmark_filename = get_bookmark_filename(
            input_filename,
            bookmark_directory.as_deref(),
            config.data_directory.as_deref(),
        );
        info!(
            "Using bookmark filename {:?} for input {:?}",
            bookmark_filename, input_filename
        );

        let importer = if let Some(importer) = context.datastore.get_importer() {
            importer
        } else {
            error!("No importer implementation for this database.");
            std::process::exit(1);
        };

        let filters = vec![
            crate::eve::filters::EveFilter::Filters(shared_filters.clone()),
            EveBoxMetadataFilter {
                filename: Some(input_filename.clone()),
            }
            .into(),
        ];

        let reader = EveReader::new(input_filename);
        let mut processor = Processor::new(reader, importer.clone());
        processor.report_interval = Duration::from_secs(60);
        processor.filters = Arc::new(filters);
        processor.end = end;
        processor.bookmark_filename = bookmark_filename;
        info!("Starting reader for {}", input_filename);
        tokio::spawn(async move {
            processor.run().await;
        });
    }

    let context = Arc::new(context);

    info!(
        "Starting server on {}:{}, tls={}",
        config.host, config.port, config.tls_enabled
    );
    if config.tls_enabled {
        debug!("TLS key filename: {:?}", config.tls_key_filename);
        debug!("TLS cert filename: {:?}", config.tls_cert_filename);
        run_axum_server_with_tls(&config, context).await.unwrap();
    } else {
        run_axum_server(&config, context).await.unwrap();
    }
    Ok(())
}

pub(crate) fn build_axum_service(
    context: Arc<ServerContext>,
) -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    use axum::routing::{get, post};
    use tower_http::trace::TraceLayer;

    let response_header_layer =
        tower_http::set_header::SetResponseHeaderLayer::<_, Body>::if_not_present(
            HeaderName::from_static("x-evebox-git-revision"),
            HeaderValue::from_static(crate::version::build_rev()),
        );

    let request_tracing = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().include_headers(false))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_request(());

    let app = axum::Router::new()
        .route(
            "/api/1/login",
            post(crate::server::api::login::post).options(api::login::options_new),
        )
        .route("/api/1/logout", post(crate::server::api::login::logout_new))
        .route("/api/1/config", get(api::config))
        .route("/api/1/version", get(api::get_version))
        .route("/api/1/user", get(api::get_user))
        .route("/api/1/alerts", get(api::alert_query))
        .route("/api/1/event-query", get(api::event_query))
        .route("/api/1/event/:id", get(api::get_event_by_id))
        .route("/api/1/alert-group/star", post(api::alert_group_star))
        .route("/api/1/alert-group/unstar", post(api::alert_group_unstar))
        .route("/api/1/alert-group/archive", post(api::alert_group_archive))
        .route("/api/1/alert-group/comment", post(api::alert_group_comment))
        .route("/api/1/event/:id/archive", post(api::archive_event_by_id))
        .route("/api/1/event/:id/escalate", post(api::escalate_event_by_id))
        .route("/api/1/event/:id/comment", post(api::comment_by_event_id))
        .route(
            "/api/1/event/:id/de-escalate",
            post(api::deescalate_event_by_id),
        )
        .route("/api/1/report/agg", get(api::agg))
        .route("/api/1/report/histogram", get(api::histogram))
        .route("/api/1/query", post(api::query_elastic))
        .route("/api/1/flow/histogram", get(api::flow_histogram::handler))
        .route("/api/1/report/dhcp/:what", get(api::report_dhcp))
        .route("/api/1/eve2pcap", post(api::eve2pcap::handler))
        .route("/api/1/submit", post(api::submit::handler_new))
        .layer(AddExtensionLayer::new(context.clone()))
        .layer(response_header_layer)
        .fallback(axum::routing::get(fallback_handler));

    let app = if context.config.http_request_logging {
        app.layer(request_tracing)
    } else {
        app
    };

    let service: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
        app.into_make_service_with_connect_info();
    service
}

pub(crate) async fn build_axum_server(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> Result<Server<AddrIncoming, IntoMakeServiceWithConnectInfo<Router, SocketAddr>>> {
    let port: u16 = config.port.parse()?;
    let addr: SocketAddr = format!("{}:{}", config.host, port).parse()?;
    let service = build_axum_service(context);
    info!("Starting Axum server on {}", &addr);
    let server = axum::Server::try_bind(&addr)?.serve(service);
    Ok(server)
}

pub(crate) async fn run_axum_server_with_tls(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> anyhow::Result<()> {
    let port: u16 = config.port.parse()?;
    let addr: SocketAddr = format!("{}:{}", config.host, port).parse()?;
    let service = build_axum_service(context.clone());
    use axum_server::tls_rustls::RustlsConfig;
    let tls_config = RustlsConfig::from_pem_file(
        config.tls_cert_filename.as_ref().unwrap(),
        config.tls_key_filename.as_ref().unwrap(),
    )
    .await?;
    axum_server::bind_rustls(addr, tls_config)
        .serve(service)
        .await?;
    Ok(())
}

pub(crate) async fn run_axum_server(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> anyhow::Result<()> {
    let port: u16 = config.port.parse()?;
    let addr: SocketAddr = format!("{}:{}", config.host, port).parse()?;
    let service = build_axum_service(context.clone());
    axum_server::bind(addr).serve(service).await?;
    Ok(())
}

async fn fallback_handler(uri: Uri) -> impl IntoResponse {
    use axum::http::Response;

    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.starts_with("api") {
        return (StatusCode::NOT_FOUND, "api endpoint not found").into_response();
    }

    if path.is_empty() {
        path = "index.html".into();
    }
    let resource = crate::resource::Resource::get(&path).or_else(|| {
        info!("No resource found for {}, trying public/index.html", &path);
        path = "public/index.html".into();
        crate::resource::Resource::get(&path)
    });

    match resource {
        None => {
            let response = serde_json::json!({
                "error": "no resource at path",
                "path": &path,
            });
            return (StatusCode::NOT_FOUND, axum::Json(response)).into_response();
        }
        Some(body) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            return Response::builder()
                .header(axum::http::header::CONTENT_TYPE, mime.as_ref())
                .body(body.into())
                .unwrap();
        }
    }
}

pub async fn build_context(
    config: ServerConfig,
    datastore: Option<Datastore>,
) -> anyhow::Result<ServerContext> {
    let config_repo = if let Some(directory) = &config.data_directory {
        let filename = PathBuf::from(directory).join("config.sqlite");
        info!("Configuration database filename: {:?}", filename);
        ConfigRepo::new(Some(&filename))?
    } else {
        info!("Using temporary in-memory configuration database");
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

async fn wait_for_version(client: &Client) -> elastic::client::Version {
    loop {
        match client.get_version().await {
            Err(err) => {
                warn!(
                    "Failed to get Elasticsearch version, will try again: {}",
                    err
                );
            }
            Ok(version) => {
                return version;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
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

            let version = wait_for_version(&client).await;
            if version.major < 6 {
                return Err(anyhow!(
                    "Elasticsearch versions less than 6 are not supported"
                ));
            }
            info!(
                "Found Elasticsearch version {} at {}",
                version.version, &config.elastic_url
            );

            let index_pattern = if config.elastic_no_index_suffix {
                config.elastic_index.clone()
            } else {
                format!("{}-*", config.elastic_index)
            };

            let eventstore = elastic::EventStore {
                base_index: config.elastic_index.clone(),
                index_pattern: index_pattern,
                client: client,
                ecs: config.elastic_ecs,
                no_index_suffix: config.elastic_no_index_suffix,
            };
            debug!("Elasticsearch base index: {}", &eventstore.base_index);
            debug!(
                "Elasticsearch search index pattern: {}",
                &eventstore.index_pattern
            );
            debug!("Elasticsearch ECS mode: {}", eventstore.ecs);
            context.features.reporting = true;
            context.features.comments = true;
            context.elastic = Some(eventstore.clone());
            context.datastore = Datastore::Elastic(eventstore);
        }
        "sqlite" => {
            let db_filename = if let Some(dir) = &config.data_directory {
                std::path::PathBuf::from(dir).join("events.sqlite")
            } else if let Some(filename) = &config.sqlite_filename {
                std::path::PathBuf::from(filename)
            } else {
                panic!("data-directory required");
            };
            let connection_builder =
                Arc::new(sqlite::ConnectionBuilder::filename(Some(db_filename)));
            let connection = connection_builder.open()?;
            sqlite::init_event_db(&mut connection_builder.open()?)?;
            let connection = Arc::new(Mutex::new(connection));

            let eventstore = sqlite::eventstore::SQLiteEventStore::new(connection_builder);
            context.datastore = Datastore::SQLite(eventstore);

            // Setup retention job.
            if let Some(period) = config.database_retention_period {
                if period > 0 {
                    info!("Setting data retention period to {} days", period);
                    let retention_config = sqlite::retention::RetentionConfig { days: period };
                    let connection = connection;
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

#[derive(Debug)]
pub enum GenericError {
    NotFound,
    AuthenticationRequired,
}

#[derive(Debug)]
pub(crate) struct AxumSessionExtractor(pub(crate) Arc<Session>);

#[async_trait]
impl FromRequest for AxumSessionExtractor {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: &mut RequestParts<Body>) -> Result<Self, Self::Rejection> {
        let axum::extract::Extension(context) =
            axum::extract::Extension::<Arc<ServerContext>>::from_request(req)
                .await
                .unwrap();
        let enable_reverse_proxy = context.config.http_reverse_proxy;
        let Extension(ConnectInfo(remote_addr)) =
            axum::extract::Extension::<ConnectInfo<SocketAddr>>::from_request(req)
                .await
                .unwrap();
        let headers = req.headers().expect("other extractor taken headers");

        let session_id = headers
            .get("x-evebox-session-id")
            .map(|h| h.to_str().ok())
            .flatten();

        let remote_user = headers
            .get("remote_user")
            .map(|h| h.to_str().map(|h| h.to_string()).ok())
            .flatten();

        let forwarded_for = if enable_reverse_proxy {
            headers
                .get("x-forwarded-for")
                .map(|h| h.to_str().map(|h| h.to_string()).ok())
                .flatten()
        } else {
            None
        };

        let _remote_addr = forwarded_for.unwrap_or_else(|| remote_addr.to_string());

        if let Some(session_id) = session_id {
            let session = context.session_store.get(session_id);
            if let Some(session) = session {
                return Ok(AxumSessionExtractor(session));
            }
        }

        match context.config.authentication_type {
            AuthenticationType::Anonymous => {
                return Ok(Self(Arc::new(Session::anonymous(remote_user))));
            }
            _ => {
                // Any authentication type requires a session.
                info!("Authentication required but not session found.");
                return Err((
                    axum::http::StatusCode::UNAUTHORIZED,
                    "authentication required",
                ));
            }
        }
    }
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
        warn!(
            "Found legacy bookmark file, checking if writable: {}",
            &legacy_filename
        );
        match test_writable(&legacy_filename) {
            Ok(_) => {
                warn!("Using legacy bookmark filename: {}", &legacy_filename);
                return Some(PathBuf::from(&legacy_filename));
            }
            Err(err) => {
                error!(
                    "Legacy bookmark filename not writable, will not use: filename={}, error={}",
                    legacy_filename, err
                );
            }
        }
    }

    // Do we have a global data-directory, and is it writable?
    if let Some(directory) = data_directory {
        let bookmark_filename = bookmark::bookmark_filename(input_filename, directory);
        debug!("Checking {:?} for writability", &bookmark_filename);
        if let Err(err) = test_writable(&bookmark_filename) {
            error!("{:?} not writable: {}", &bookmark_filename, err);
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
