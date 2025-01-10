// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use super::{ServerConfig, ServerContext};
use crate::bookmark;
use crate::config::Config;
use crate::elastic;
use crate::elastic::Version;
use crate::eve::filters::{AddFieldFilter, AddRuleFilter};
use crate::eve::watcher::EvePatternWatcher;
use crate::eventrepo::EventRepo;
use crate::server::api;
use crate::server::session::Session;
use crate::sqlite::configrepo::{self, ConfigRepo};
use crate::sqlite::connection::init_event_db;
use crate::sqlite::{self};
use anyhow::Result;
use axum::async_trait;
use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Extension, FromRequestParts};
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Router;
use axum_extra::extract::CookieJar;
use axum_extra::TypedHeader;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse};
use tracing::{debug, error, info, warn, Level};

fn load_event_services(filename: &str) -> Result<serde_json::Value> {
    let finput = std::fs::File::open(filename)?;
    let yaml_value: serde_yaml::Value = serde_yaml::from_reader(finput)?;
    let json_value = serde_json::to_value(&yaml_value["event-services"])?;
    Ok(json_value)
}

#[allow(clippy::field_reassign_with_default)]
pub async fn main(args: &clap::ArgMatches) -> Result<()> {
    crate::version::log_version();

    // Load the configuration file if provided.
    let config_filename = args.get_one::<String>("config").map(|s| &**s);
    let config = match crate::config::Config::new(args.clone(), config_filename) {
        Err(err) => {
            error!(
                "Failed to load configuration: {:?} - filename={:?}",
                err, config_filename
            );
            std::process::exit(1);
        }
        Ok(config) => config,
    };

    let mut server_config = ServerConfig::default();

    // Database type. Specific datastore type command line option
    // takes precedence.
    if args.get_flag("sqlite") {
        server_config.datastore = "sqlite".to_string();
    } else {
        server_config.datastore = config.get("database.type")?.unwrap();
    }

    server_config.port = config.get("http.port")?.unwrap();
    server_config.host = config.get("http.host")?.unwrap();
    server_config.tls_enabled = config.get_bool("http.tls.enabled")?;
    server_config.tls_cert_filename = config.get("http.tls.certificate")?;
    server_config.tls_key_filename = config.get("http.tls.key")?;
    server_config.elastic_url = config.get("database.elasticsearch.url")?.unwrap();
    server_config.elastic_index = config.get("database.elasticsearch.index")?.unwrap();
    server_config.elastic_no_index_suffix =
        config.get_bool("database.elasticsearch.no-index-suffix")?;
    server_config.elastic_ecs = config.get_bool("database.elasticsearch.ecs")?;
    server_config.elastic_username = config.get("database.elasticsearch.username")?;
    server_config.elastic_password = config.get("database.elasticsearch.password")?;
    server_config.data_directory = config.get("data-directory")?;
    server_config.no_check_certificate = config
        .get_bool("database.elasticsearch.disable-certificate-check")?
        || config.get_bool("no-check-certificate")?;
    server_config.http_request_logging = config.get_bool("http.request-logging")?;
    server_config.http_reverse_proxy = config.get_bool("http.reverse-proxy")?;

    debug!(
        "Certificate checks disabled: {}",
        server_config.no_check_certificate,
    );

    server_config.authentication_required = is_authentication_required(&config);

    // Do we need a data-directory? If so, make sure its set.
    let data_directory_required = server_config.datastore == "sqlite"
        || server_config.authentication_required
        || (server_config.tls_enabled
            && server_config.tls_key_filename.is_none()
            && server_config.tls_cert_filename.is_none());

    // TODO: A data directory should always be preferred, even if not
    // required as we store stuff like the JA4db in the configuration
    // database.
    if server_config.data_directory.is_none() {
        let dd = crate::config::get_data_directory(None);
        info!("Using (discovered) data-directory {}", dd.display());
        server_config.data_directory = Some(dd.display().to_string());
    } else if data_directory_required {
        info!(
            "Using data directory {}",
            server_config.data_directory.as_ref().unwrap()
        );
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register CTRL-C handler");
        std::process::exit(0);
    });

    let datastore = configure_datastore(config.clone(), &server_config).await?;

    let config_repo = if let Some(directory) = &server_config.data_directory {
        let filename = PathBuf::from(directory).join("config.sqlite");
        info!("Configuration database filename: {:?}", filename);
        configrepo::open(Some(&filename)).await?
    } else {
        info!("Using temporary in-memory configuration database");
        configrepo::open(None).await?
    };

    let mut context = build_context(server_config.clone(), datastore, config_repo).await?;

    if server_config.authentication_required && !context.config_repo.has_users().await? {
        warn!("Username/password authentication is required, but no users exist, creating a user");
        let (username, password) = create_admin_user(&context).await?;
        warn!(
            "Created administrator username and password: username={}, password={}",
            username, password
        );
    }

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

    if is_input_enabled(&config) {
        let input_patterns = get_input_patterns(&config)?;
        if input_patterns.is_empty() {
            bail!("EVE input enabled, but no paths provided");
        }
        let sink = context.datastore.get_importer().ok_or(anyhow!(
            "An event importer is not implemented for this datastore"
        ))?;

        let mut filters = Vec::new();

        match config.get_config_value::<Vec<String>>("input.rules") {
            Ok(Some(rules)) => {
                let rulemap = crate::rules::load_rules(&rules);
                let rulemap = Arc::new(rulemap);
                filters.push(crate::eve::filters::EveFilter::AddRuleFilter(
                    AddRuleFilter {
                        map: rulemap.clone(),
                    },
                ));
                crate::rules::watch_rules(rulemap);
            }
            Ok(None) => {}
            Err(err) => {
                error!("Failed to read input.rules configuration: {}", err);
            }
        }

        let geoip_disabled = config.get_bool("geoip.disabled")?;
        if geoip_disabled {
            debug!("GeoIP disabled");
        } else {
            let geoip_database = config.get_string("geoip.database");
            match crate::geoip::GeoIP::open(geoip_database) {
                Ok(db) => {
                    filters.push(crate::eve::filters::EveFilter::GeoIP(db));
                }
                Err(err) => {
                    warn!("Failed to open GeoIP database: error={}", err);
                }
            }
        }

        filters.push(crate::eve::filters::EveFilter::AutoArchiveFilter(
            crate::eve::filters::AutoArchiveFilter::default(),
        ));

        let additional_fields: Option<HashMap<String, serde_yaml::Value>> =
            config.get_value("input.additional-fields")?;
        if let Some(fields) = additional_fields {
            for (k, v) in fields {
                let v = serde_json::from_str(&serde_json::to_string(&v)?)?;
                let filter = AddFieldFilter::new(k, v);
                filters.push(crate::eve::filters::EveFilter::AddFieldFilter(filter));
            }
        }

        let end = config.get_bool("end")?;

        let bookmark_directory: Option<String> = config.get_string("input.bookmark-directory");
        let data_directory = server_config.data_directory.clone();
        let watcher = EvePatternWatcher::new(
            input_patterns,
            sink,
            filters,
            end,
            bookmark_directory,
            data_directory,
        );
        watcher.run();
    }

    let context = Arc::new(context);
    info!(
        "Starting server on {}:{}, tls={}",
        server_config.host, server_config.port, server_config.tls_enabled
    );
    if server_config.tls_enabled {
        if server_config.tls_key_filename.is_none() && server_config.tls_cert_filename.is_none() {
            // No TLS certificate or key filenames have been provided,
            // generate self signed certificates.
            let tls_dir = if let Some(dir) = &server_config.data_directory {
                PathBuf::from(dir)
            } else {
                error!("Unable to determine what directory to store TLS certificate and key in, please provide a data directory or start with --no-tls to disable TLS");
                std::process::exit(1);
            };
            info!(
                "Using directory {} for self signed TLS certificate and key files",
                tls_dir.display()
            );

            crate::path::ensure_exists(&tls_dir)?;
            let (cert_path, key_path) = crate::cert::get_or_create_cert(tls_dir)?;
            server_config.tls_cert_filename = Some(cert_path);
            server_config.tls_key_filename = Some(key_path);
        }
        debug!("TLS key filename: {:?}", server_config.tls_key_filename);
        debug!("TLS cert filename: {:?}", server_config.tls_cert_filename);
        if let Err(err) = run_axum_server_with_tls(&server_config, context).await {
            error!("Failed to start TLS HTTP service: {:?}", err);
            std::process::exit(1);
        }
    } else if let Err(err) = run_axum_server(&server_config, context).await {
        error!("Failed to start HTTP service: {:?}", err);
        std::process::exit(1);
    }
    Ok(())
}

fn is_authentication_required(config: &Config) -> bool {
    // Allows `authentication: false` to disable authentication.
    if let Ok(Some(false)) = config.get::<bool>("authentication") {
        info!("Authentication disabled by configuration: authentication");
        return false;
    }

    // First check if authentication has been explicitly disabled.
    if !config.get_bool_with_default("authentication.required", true) {
        info!("Authentication disabled by configuration: authentication.required");
        return false;
    }

    // Default to true.
    true
}

async fn create_admin_user(context: &ServerContext) -> Result<(String, String)> {
    use rand::Rng;
    let rng = rand::thread_rng();
    let username = "admin";
    let password: String = rng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    context.config_repo.add_user(username, &password).await?;
    Ok((username.to_string(), password))
}

fn is_input_enabled(config: &Config) -> bool {
    config.args.contains_id("input.filename")
        || config.args.contains_id("input.paths")
        || config.get_bool("input.enabled").unwrap_or(false)
}

fn get_input_patterns(config: &Config) -> Result<Vec<String>> {
    let mut input_pattern_set = HashSet::new();

    if let Some(filename) = config.get::<String>("input.filename")? {
        input_pattern_set.insert(filename);
    }

    if let Some(paths) = config.get_many::<String>("input.paths")? {
        for path in &paths {
            input_pattern_set.insert(path.clone());
        }
    }

    let input_patterns: Vec<String> = input_pattern_set.iter().map(|s| s.to_string()).collect();
    Ok(input_patterns)
}

pub(crate) fn build_axum_service(
    context: Arc<ServerContext>,
) -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    let response_header_layer = tower_http::set_header::SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static("x-evebox-git-revision"),
        HeaderValue::from_static(crate::version::build_rev()),
    );

    let request_tracing = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().include_headers(false))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_request(());

    let app = axum::Router::new()
        .merge(api::router())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 32))
        .layer(Extension(context.clone()))
        .layer(response_header_layer)
        .with_state(context.clone())
        .fallback(fallback_handler);

    let app = if context.config.http_request_logging {
        app.layer(request_tracing)
    } else {
        app
    };

    let service: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
        app.into_make_service_with_connect_info();
    service
}

pub(crate) async fn run_axum_server_with_tls(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> Result<()> {
    let port: u16 = config.port;
    let addr: SocketAddr = format!("{}:{}", config.host, port).parse()?;
    let service = build_axum_service(context.clone());
    use axum_server::tls_rustls::RustlsConfig;
    let tls_config = RustlsConfig::from_pem_file(
        config.tls_cert_filename.as_ref().unwrap(),
        config.tls_key_filename.as_ref().unwrap(),
    )
    .await
    .map_err(|err| {
        anyhow!(
            "Failed to load certificate or key file ({:?}, {:?}) {:?}",
            config.tls_cert_filename,
            config.tls_key_filename,
            err
        )
    })?;
    axum_server::bind_rustls(addr, tls_config)
        .serve(service)
        .await?;
    Ok(())
}

pub(crate) async fn run_axum_server(
    config: &ServerConfig,
    context: Arc<ServerContext>,
) -> Result<()> {
    let port: u16 = config.port;
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
        path = "webapp/index.html".into();
    } else {
        path = format!("webapp/{path}");
    }
    let resource = crate::resource::Resource::get(&path).or_else(|| {
        debug!("No resource found for {}, trying webapp/index.html", &path);
        path = "webapp/index.html".into();
        crate::resource::Resource::get(&path)
    });

    match resource {
        None => {
            let response = serde_json::json!({
                "error": "no resource at path",
                "path": &path,
            });
            (StatusCode::NOT_FOUND, axum::Json(response)).into_response()
        }
        Some(body) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, mime.as_ref())
                .body(axum::body::Body::from(body.data))
                .unwrap()
                .into_response()
        }
    }
}

pub(crate) async fn build_context(
    config: ServerConfig,
    datastore: EventRepo,
    config_repo: ConfigRepo,
) -> Result<ServerContext> {
    let context = ServerContext::new(config, Arc::new(config_repo), datastore);
    Ok(context)
}

async fn configure_datastore(config: Config, server_config: &ServerConfig) -> Result<EventRepo> {
    match server_config.datastore.as_ref() {
        "elasticsearch" => {
            let mut client = elastic::ClientBuilder::new(&server_config.elastic_url);
            if let Some(username) = &server_config.elastic_username {
                client = client.with_username(username);
            }
            if let Some(password) = &server_config.elastic_password {
                client = client.with_password(password);
            }
            client = client.disable_certificate_validation(server_config.no_check_certificate);

            let client = client.build();

            let server_info = client.wait_for_info().await;
            if matches!(
                server_info.version.distribution.as_deref(),
                Some("opensearch")
            ) {
                info!("Found Opensearch version {}", &server_info.version.number);
                if let Ok(version) = Version::parse(&server_info.version.number) {
                    if version.major < 2 || (version.major < 3 && version.minor < 6) {
                        error!("Opensearch versions less than 2.6.0 not supported. EveBox likely won't work properly.");
                    }
                } else {
                    error!("Failed to parse Opensearch version, EveBox likely won't work properly");
                }
                warn!("Opensearch support is still a work in progress");
            } else {
                info!(
                    "Found Elasticsearch version {}; Index={}; ECS={}",
                    &server_info.version.number,
                    server_config.elastic_index,
                    server_config.elastic_ecs,
                );
                if let Ok(version) = Version::parse(&server_info.version.number) {
                    if version.major < 7 || (version.major < 8 && version.minor < 10) {
                        error!("Elasticsearch versions less than 7.10 not support. EveBox likely won't work properly.");
                    }
                } else {
                    error!(
                        "Failed to parse Elasticsearch version, EveBox likely won't work properly"
                    );
                }
            }

            let index_pattern = if server_config.elastic_no_index_suffix {
                server_config.elastic_index.clone()
            } else {
                format!("{}-*", server_config.elastic_index)
            };

            let eventstore = elastic::ElasticEventRepo {
                base_index: server_config.elastic_index.clone(),
                index_pattern,
                client: client.clone(),
                ecs: server_config.elastic_ecs,
            };
            debug!("Elasticsearch base index: {}", &eventstore.base_index);
            debug!(
                "Elasticsearch search index pattern: {}",
                &eventstore.index_pattern
            );
            debug!("Elasticsearch ECS mode: {}", eventstore.ecs);

            elastic::util::check_and_set_field_limit(&client, &eventstore.base_index).await;

            Ok(EventRepo::Elastic(eventstore))
        }
        "sqlite" => {
            let db_filename = if let Some(dir) = &server_config.data_directory {
                crate::path::ensure_exists(dir)?;
                std::path::PathBuf::from(dir).join("events.sqlite")
            } else {
                panic!("data-directory required");
            };
            let connection_builder = Arc::new(sqlite::ConnectionBuilder::filename(Some(
                db_filename.clone(),
            )));

            let mut conn = connection_builder.open_connection(true).await.unwrap();
            init_event_db(&mut conn).await?;
            let writer = Arc::new(tokio::sync::Mutex::new(conn));
            let pool = sqlite::connection::open_pool(Some(&db_filename), false).await?;
            let rusqlite_writer = connection_builder.open_with_rusqlite().unwrap();
            let rusqlite_writer = Arc::new(Mutex::new(rusqlite_writer));
            let eventstore = sqlite::eventrepo::SqliteEventRepo::new(
                writer.clone(),
                pool,
                Some(rusqlite_writer),
            );

            // Start retention task.
            sqlite::retention::start_retention_task(config.clone(), writer.clone(), db_filename)
                .await?;
            info!("Retention task started");

            Ok(EventRepo::SQLite(eventstore))
        }
        _ => panic!("unsupported datastore"),
    }
}

#[derive(Debug)]
pub(crate) struct SessionExtractor(pub(crate) Arc<Session>);

#[async_trait]
impl<S> FromRequestParts<S> for SessionExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        req: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Extension(context) = Extension::<Arc<ServerContext>>::from_request_parts(req, state)
            .await
            .unwrap();
        let enable_reverse_proxy = context.config.http_reverse_proxy;
        let Extension(ConnectInfo(remote_addr)) =
            Extension::<ConnectInfo<SocketAddr>>::from_request_parts(req, state)
                .await
                .unwrap();
        let headers = &req.headers;

        let cookies = CookieJar::from_headers(headers);
        let session_id = cookies
            .get("x-evebox-session-id")
            .map(|c| c.value().to_string());

        let remote_user = headers
            .get("remote_user")
            .and_then(|h| h.to_str().map(|h| h.to_string()).ok());

        let forwarded_for = if enable_reverse_proxy {
            headers
                .get("x-forwarded-for")
                .and_then(|h| h.to_str().map(|h| h.to_string()).ok())
        } else {
            None
        };

        // TODO: Proper reverse proxy handling, mainly for logging.
        let _remote_addr = forwarded_for.unwrap_or_else(|| remote_addr.to_string());

        if let Some(session_id) = session_id {
            let session = context.session_store.get(&session_id);
            if let Some(session) = session {
                return Ok(SessionExtractor(session));
            }

            debug!("Session not found in cache, checking database");

            match context.config_repo.get_user_by_session(&session_id).await {
                Ok(Some(user)) => {
                    info!("Found session for user {}", &user.username);
                    let session = Session {
                        session_id: Some(session_id.to_string()),
                        username: Some(user.username),
                    };
                    let session = Arc::new(session);
                    let _ = context.session_store.put(session.clone());
                    return Ok(SessionExtractor(session));
                }
                Ok(None) => {}
                Err(err) => {
                    error!("Failed to get user by session from database: {:?}", err);
                }
            }
        }

        use axum_extra::headers::authorization::Basic;
        use axum_extra::headers::Authorization;

        let authorization = if headers.contains_key("authorization") {
            let TypedHeader(Authorization(basic)) =
                TypedHeader::<Authorization<Basic>>::from_request_parts(req, state)
                    .await
                    .map_err(|err| {
                        warn!("Failed to decode basic authentication header: {:?}", err);
                        (StatusCode::UNAUTHORIZED, "bad authorization header")
                    })?;
            Some(basic)
        } else {
            None
        };

        if context.config.authentication_required {
            if let Some(basic) = authorization {
                match context
                    .config_repo
                    .get_user_by_username_password(basic.username(), basic.password())
                    .await
                {
                    Ok(user) => {
                        return Ok(Self(Arc::new(Session::with_username(&user.username))));
                    }
                    Err(err) => {
                        warn!(
                            "Basic authentication failure for username {}, error={:?}",
                            basic.username(),
                            err
                        );
                    }
                }
            }
            info!("Authentication required but no session found.");
        } else {
            return Ok(Self(Arc::new(Session::anonymous(remote_user))));
        }

        return Err((StatusCode::UNAUTHORIZED, "authentication required"));
    }
}

pub(crate) fn get_bookmark_filename<P: AsRef<Path> + Clone>(
    input_filename: P,
    input_bookmark_dir: Option<&str>,
    data_directory: Option<&str>,
) -> Option<PathBuf> {
    // First priority is the input_bookmark_directory.
    if let Some(directory) = input_bookmark_dir {
        return Some(bookmark::bookmark_filename(input_filename, directory));
    }

    // Otherwise see if there is a file with the same name as the input filename but
    // suffixed with ".bookmark".
    let legacy_filename = format!("{}.bookmark", input_filename.as_ref().display());
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
        let bookmark_filename = bookmark::bookmark_filename(input_filename.clone(), directory);
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
