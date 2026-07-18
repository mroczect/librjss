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
    let decoded = urlencoding::decode(path)
        .map_err(|_| JssError::Validation("Invalid URL path encoding".into()))?;

    if decoded.contains("://") {
        return Err(JssError::Validation("Absolute URL not allowed".into()));
    }

    if decoded.contains("..") {
        return Err(JssError::Validation("Path traversal not allowed".into()));
    }

    let joined = client
        .config
        .base_url
        .join(&decoded)
        .map_err(|e| JssError::Parse(format!("URL join error: {e}")))?;

    client.config.validate_joined_url(&joined)?;

    Ok(joined)
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

pub(crate) fn classify_response(status: StatusCode, body: &str) -> backoff::Error<JssError> {
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        let error = JssError::from_api_response(status, body);
        backoff::Error::transient(error)
    } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        let error = JssError::from_api_response(status, body);
        backoff::Error::permanent(error)
    } else if matches!(
        status,
        StatusCode::BAD_REQUEST
            | StatusCode::NOT_FOUND
            | StatusCode::METHOD_NOT_ALLOWED
            | StatusCode::CONFLICT
            | StatusCode::GONE
    ) {
        backoff::Error::permanent(JssError::Http {
            status,
            body: body.to_string(),
        })
    } else {
        backoff::Error::transient(JssError::Http {
            status,
            body: body.to_string(),
        })
    }
}
