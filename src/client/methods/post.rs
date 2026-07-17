use crate::client::RjssClient;
use crate::client::auth::http_helpers::{apply_auth_to_builder, backoff_config, build_join_url};
use crate::handler::error::JssError;
use backoff::future::retry;
use reqwest::StatusCode;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use tracing::debug;

pub(crate) async fn authenticated_post(
    client: &RjssClient,
    path: &str,
    body_json: &str,
) -> Result<String, JssError> {
    let url = build_join_url(client, path)?;
    let http = client.http.clone();
    let auth_mode = client.config.auth_mode.clone();
    let backoff = backoff_config(client);
    let csrf_token = client
        .session
        .as_ref()
        .map(|s| s.csrf_token.expose_secret().to_owned());
    let is_session = matches!(&auth_mode, crate::handler::config::AuthMode::Session { .. });
    let trace_id = client.trace_id.clone();

    let op = || {
        let http = http.clone();
        let auth_mode = auth_mode.clone();
        let url = url.clone();
        let body = body_json.to_owned();
        let csrf = csrf_token.clone();
        async move {
            let mut req = http
                .post(url)
                .body(body)
                .header("Content-Type", "application/json");
            if is_session {
                if let Some(token) = &csrf {
                    if !token.is_empty() {
                        req = req.header("X-Frappe-CSRF-Token", token);
                    }
                }
            }
            req = apply_auth_to_builder(&auth_mode, req);
            let resp = req
                .send()
                .await
                .map_err(|e| backoff::Error::transient(JssError::Network(e)))?;
            let status = resp.status();
            if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                Err(backoff::Error::transient(JssError::Http {
                    status,
                    body: String::new(),
                }))
            } else if status == StatusCode::UNAUTHORIZED {
                Err(backoff::Error::permanent(JssError::Auth(
                    "Unauthorized".into(),
                )))
            } else {
                Ok(resp)
            }
        }
    };

    let resp = retry(backoff, op).await?;
    let status = resp.status();
    let body = resp.text().await?;
    debug!(trace_id = %trace_id, path = %path, status = %status, body_hash = format!("{:x}", Sha256::digest(body.as_bytes())));
    Ok(body)
}
