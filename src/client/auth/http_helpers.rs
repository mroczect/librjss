use crate::client::RjssClient;
use crate::handler::config::AuthMode;
use crate::handler::error::JssError;
use backoff::ExponentialBackoff;
use reqwest::RequestBuilder;
use reqwest::Response;
use reqwest::StatusCode;
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

pub(crate) fn build_join_url(client: &RjssClient, path: &str) -> Result<reqwest::Url, JssError> {
    if path.contains("..") {
        return Err(JssError::Validation("Path traversal not allowed".into()));
    }
    client
        .config
        .base_url
        .join(path)
        .map_err(|e| JssError::Parse(format!("URL join error: {e}")))
}

pub(crate) fn backoff_config(client: &RjssClient) -> ExponentialBackoff {
    ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(
            client.config.timeout_secs * (client.config.max_retries as u64 + 1),
        )),
        ..Default::default()
    }
}

#[allow(dead_code)]
pub(crate) fn extract_retry_after(response: &Response) -> Option<u64> {
    response
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
}

#[allow(dead_code)]
pub(crate) fn classify_response(status: StatusCode, body: &str) -> backoff::Error<JssError> {
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        let error = JssError::from_api_response(status, body);
        backoff::Error::transient(error)
    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        let error = JssError::from_api_response(status, body);
        backoff::Error::permanent(error)
    } else {
        backoff::Error::transient(JssError::Http {
            status,
            body: body.to_string(),
        })
    }
}
