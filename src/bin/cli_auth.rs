use librjss::RjssClient;
use librjss::handler::env::{AuthMode, ClientConfig};
use secrecy::SecretString;
use std::env;
use tracing::info;
use tracing_subscriber::filter::LevelFilter;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .with_target(false)
        .init();

    let base_url = env::var("JSS_BASE_URL").expect("JSS_BASE_URL not set");
    let email = env::var("JSS_EMAIL").ok();
    let password = env::var("JSS_PASSWORD").ok();
    let api_key = env::var("JSS_TOKEN_API_KEY").ok();
    let api_secret = env::var("JSS_TOKEN_API_SECRET").ok();

    let auth_mode = if let (Some(key), Some(secret)) = (api_key, api_secret) {
        info!("Using token authentication");
        AuthMode::Token {
            api_key: key,
            api_secret: SecretString::new(Box::from(secret)),
        }
    } else if let (Some(em), Some(pw)) = (email, password) {
        info!("Using session authentication");
        AuthMode::Session {
            email: SecretString::new(Box::from(em)),
            password: SecretString::new(Box::from(pw)),
        }
    } else {
        eprintln!(
            "Either JSS_EMAIL+JSS_PASSWORD or JSS_TOKEN_API_KEY+JSS_TOKEN_API_SECRET must be set"
        );
        std::process::exit(1);
    };

    let expected_sitename = env::var("JSS_EXPECTED_SITENAME").ok();
    let required_roles: Vec<String> = env::var("JSS_REQUIRED_ROLES")
        .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
        .unwrap_or_default();
    let timeout_secs: u64 = env::var("JSS_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let max_retries: u32 = env::var("JSS_MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let user_agent = env::var("JSS_USER_AGENT").unwrap_or_else(|_| "cli-auth-test/1.0".into());
    let insecure_ssl = env::var("JSS_INSECURE_SSL")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let base_url = Url::parse(&base_url).expect("Invalid JSS_BASE_URL");
    let config = ClientConfig {
        base_url,
        auth_mode,
        expected_sitename,
        required_roles,
        timeout_secs,
        max_retries,
        user_agent,
        insecure_ssl,
    };

    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {e}");
        std::process::exit(1);
    }

    let mut client = RjssClient::new(config)?;
    info!("Client created (trace_id: {})", client.trace_id());

    match client.authenticate().await {
        Ok(()) => {
            info!("Authentication successful");
            if let Some(info) = client.session_info() {
                info!("Full name: {:?}", info.full_name);
                info!("Sitename: {}", info.sitename);
                info!("Roles: {:?}", info.roles);
            }
            match client
                .authenticated_get("/api/method/frappe.auth.get_logged_user")
                .await
            {
                Ok(body) => info!("Test request succeeded, body length: {}", body.len()),
                Err(e) => {
                    eprintln!("Test request failed: {e}");
                    std::process::exit(2);
                }
            }
        }
        Err(e) => {
            eprintln!("Authentication failed: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
