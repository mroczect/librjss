use crate::client::RjssClient;
use crate::handler::config::AuthMode;
use crate::handler::error::JuraganError;
use backoff::ExponentialBackoff;
use reqwest::RequestBuilder;
use secrecy::ExposeSecret;
use std::time::Duration;

pub(crate) fn apply_auth_to_builder(
    auth_mode: &AuthMode,
    mut req: RequestBuilder,
) -> RequestBuilder {
    if let AuthMode::Token {
        api_key,
        api_secret,
    } = auth_mode
    {
        req = req.header(
            "Authorization",
            format!("token {}:{}", api_key, api_secret.expose_secret()),
        );
    }
    req
}

pub(crate) fn build_join_url(client: &RjssClient, path: &str) -> Result<reqwest::Url, JuraganError> {
    if path.contains("..") {
        return Err(JuraganError::Validation(
            "Path traversal not allowed".into(),
        ));
    }
    client
        .config
        .base_url
        .join(path)
        .map_err(|e| JuraganError::Parse(format!("URL join error: {e}")))
}

pub(crate) fn backoff_config(client: &RjssClient) -> ExponentialBackoff {
    ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(
            client.config.timeout_secs * (client.config.max_retries as u64 + 1),
        )),
        ..Default::default()
    }
}
