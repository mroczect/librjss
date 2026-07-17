use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use crate::handler::types::login::{LoginApiMessage, LoginApiResponse};
use crate::handler::types::session::SessionInfo;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, instrument, warn};

use super::app_parser::{extract_app_data, fetch_app_page};

#[instrument(skip(client, email, password), fields(trace_id = client.trace_id))]
pub(crate) async fn login_with_credentials(
    client: &mut RjssClient,
    email: SecretString,
    password: SecretString,
) -> Result<(), JssError> {
    let login_url = RjssClient::login_url(&client.config.base_url);

    let email_hash = format!("{:x}", Sha256::digest(email.expose_secret().as_bytes()));
    info!(trace_id = client.trace_id, email_hash = %email_hash, "Login attempt");

    let params = [
        ("usr", email.expose_secret()),
        ("pwd", password.expose_secret()),
    ];

    let resp = client.http.post(login_url).form(&params).send().await?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = resp
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        error!(trace_id = client.trace_id, "Rate limited");
        return Err(JssError::RateLimited { retry_after });
    }
    if status != reqwest::StatusCode::OK {
        let body = resp.text().await.unwrap_or_default();
        error!(trace_id = client.trace_id, %status, %body, "Login failed");
        return Err(JssError::Auth(format!("Login failed: HTTP {status}")));
    }

    let body_text = resp.text().await?;
    error!(trace_id = client.trace_id, %body_text, "Raw login response body");
    debug!(
        trace_id = client.trace_id,
        body_hash = format!("{:x}", Sha256::digest(body_text.as_bytes())),
        "Login response received"
    );

    let v: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|e| JssError::Parse(format!("Login response not valid JSON: {e}")))?;

    let login_resp = if v["message"].as_str() == Some("Logged In") {
        warn!(
            trace_id = client.trace_id,
            "Login response has string message 'Logged In'"
        );
        LoginApiResponse {
            message: LoginApiMessage {
                sid: String::new(),
                full_name: v["full_name"].as_str().map(|s| s.to_string()),
            },
        }
    } else {
        serde_json::from_value::<LoginApiResponse>(v)
            .map_err(|e| JssError::Parse(format!("Failed to parse login response: {e}")))?
    };

    let app_html = fetch_app_page(client).await?;
    let (csrf_token, boot_data) = extract_app_data(&app_html)?;

    client.boot = Some(boot_data.clone());

    let user = &boot_data.user;
    let roles = user.roles.clone();
    let full_name = if user.full_name.is_empty() {
        login_resp.message.full_name
    } else {
        Some(user.full_name.clone())
    };
    let sitename = boot_data.sitename.clone();

    if let Some(expected) = &client.config.expected_sitename {
        if expected != &sitename {
            error!(trace_id = client.trace_id, expected = %expected, actual = %sitename, "Sitename mismatch");
            return Err(JssError::SitenameMismatch {
                expected: expected.clone(),
                actual: sitename,
            });
        }
        info!(trace_id = client.trace_id, sitename = %sitename, "Sitename verified");
    }

    if !client.config.required_roles.is_empty() {
        let has_role = client
            .config
            .required_roles
            .iter()
            .any(|r| roles.contains(r));
        if !has_role {
            error!(trace_id = client.trace_id, ?roles, required = ?client.config.required_roles, "Missing required roles");
            return Err(JssError::Permission(format!(
                "Missing one of required roles: {:?}",
                client.config.required_roles
            )));
        }
    }

    client.session = Some(SessionInfo {
        sid: SecretString::new(Box::from(login_resp.message.sid)),
        csrf_token,
        full_name,
        sitename,
        roles,
    });

    info!(trace_id = client.trace_id, email_hash = %email_hash, "Login successful");
    Ok(())
}
