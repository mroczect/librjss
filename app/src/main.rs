mod config;
mod error;
mod handler;
mod providers;
mod state;

use actix_cors::Cors;
use actix_web::{App, HttpServer, web};
use dotenv::dotenv;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

use config::AppConfig;
use librjss::api::AuthManager;
use librjss::handler::session_store::MemorySessionStore;
use providers::external::{ExternalSessionStore, ExternalUserProvider};
use providers::hardcoded::HardcodedUserProvider;
use state::AppState;

#[derive(Debug, PartialEq, Clone, Copy)]
enum RunMode {
    Dev,
    Prod,
}

fn parse_mode() -> RunMode {
    if std::env::args().any(|a| a == "--dev") {
        RunMode::Dev
    } else {
        RunMode::Prod
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let mode = parse_mode();

    let log_builder = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_target(true)
        .with_level(true);
    match mode {
        RunMode::Dev => log_builder.init(),
        RunMode::Prod => log_builder.json().init(),
    }

    let app_config = match mode {
        RunMode::Dev => {
            let auth_config = librjss::handler::config::AuthConfig::builder()
                .cookie_name("session_id".into())
                .cookie_secure(false)
                .cookie_same_site(librjss::handler::config::SameSite::Lax)
                .session_lifetime(time::Duration::hours(24))
                .build()
                .expect("valid dev config");
            AppConfig {
                host: "127.0.0.1".into(),
                port: 8080,
                allowed_origins: vec![],
                auth: auth_config,
            }
        }
        RunMode::Prod => AppConfig::from_env(),
    };

    let session_store = Arc::new(MemorySessionStore::new());
    let http_client = reqwest::Client::new();

    let (user_provider, external_store): (
        Arc<dyn librjss::handler::types::UserProvider>,
        Option<Arc<ExternalSessionStore>>,
    ) = match mode {
        RunMode::Dev => (Arc::new(HardcodedUserProvider), None),
        RunMode::Prod => {
            let ext = Arc::new(ExternalSessionStore::new());
            let provider = ExternalUserProvider {
                client: http_client.clone(),
                session_store: ext.clone(),
            };
            (Arc::new(provider), Some(ext))
        }
    };

    let auth_manager = AuthManager::new(app_config.auth, user_provider, session_store);

    let state = web::Data::new(AppState {
        auth_manager,
        external_session_store: external_store,
        http_client,
    });

    info!(
        "Running in {:?} mode on {}:{}",
        mode, app_config.host, app_config.port
    );

    HttpServer::new(move || {
        let cors = if mode == RunMode::Dev {
            Cors::permissive()
        } else {
            let mut cors = Cors::default();
            for origin in &app_config.allowed_origins {
                cors = cors.allowed_origin(origin);
            }
            cors
        }
        .allowed_methods(vec!["GET", "POST"])
        .allowed_headers(vec![
            actix_web::http::header::CONTENT_TYPE,
            actix_web::http::header::COOKIE,
        ])
        .supports_credentials()
        .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .route("/login", web::post().to(handler::login::login))
            .route("/check", web::get().to(handler::check::check))
            .route("/logout", web::post().to(handler::logout::logout))
            .route(
                "/proxy/{tail:.*}",
                web::get().to(handler::proxy::proxy_collection),
            )
    })
    .bind((app_config.host.as_str(), app_config.port))?
    .run()
    .await
}
