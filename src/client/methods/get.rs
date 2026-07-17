use crate::client::RjssClient;
use crate::client::auth::http_helpers::{apply_auth_to_builder, backoff_config, build_join_url};
use crate::handler::error::JssError;
use backoff::future::retry;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use tracing::debug;

pub(crate) async fn authenticated_get(client: &RjssClient, path: &str) -> Result<String, JssError> {
    let url = build_join_url(client, path)?;
    let http = client.http.clone();
    let auth_mode = client.config.auth_mode.clone();
    let backoff = backoff_config(client);
    let trace_id = client.trace_id.clone();

    let op = || {
        let http = http.clone();
        let auth_mode = auth_mode.clone();
        let url = url.clone();
        async move {
            let mut req = http.get(url);
            req = apply_auth_to_builder(&auth_mode, req);
            let resp = req
                .send()
                .await
                .map_err(|e| backoff::Error::transient(JssError::Network(e)))?;
            let status = resp.status();
            if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                let body = resp.text().await.unwrap_or_default();
                let error = JssError::from_api_response(status, &body);
                Err(backoff::Error::transient(error))
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
