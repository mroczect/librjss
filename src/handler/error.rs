use http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JssError {
    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("HTTP {status}: {body}")]
    Http { status: StatusCode, body: String },

    #[error("API error: {exc_type}: {message}")]
    ApiError {
        exc_type: String,
        message: String,
        status: StatusCode,
    },

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("CSRF token missing or invalid")]
    Csrf(String),

    #[error("Insufficient permissions: {0}")]
    Permission(String),

    #[error("Sitename mismatch: expected {expected}, got {actual}")]
    SitenameMismatch { expected: String, actual: String },

    #[error("No active session (not authenticated)")]
    NotAuthenticated,

    #[error("Rate limited; retry after {retry_after:?}")]
    RateLimited { retry_after: Option<u64> },

    #[error("Data parsing error: {0}")]
    Parse(String),

    #[error("Token/secret expired")]
    Expired,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("File operation failed: {0}")]
    FileOperation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl JssError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            JssError::Config(_) | JssError::Validation(_) | JssError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            JssError::Network(_) => StatusCode::BAD_GATEWAY,
            JssError::Http { status, .. } | JssError::ApiError { status, .. } => *status,
            JssError::Auth(_) | JssError::NotAuthenticated | JssError::Expired => {
                StatusCode::UNAUTHORIZED
            }
            JssError::Csrf(_) => StatusCode::FORBIDDEN,
            JssError::Permission(_) => StatusCode::FORBIDDEN,
            JssError::SitenameMismatch { .. } => StatusCode::FORBIDDEN,
            JssError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            JssError::Parse(_) => StatusCode::INTERNAL_SERVER_ERROR,
            JssError::Cancelled => StatusCode::SERVICE_UNAVAILABLE,
            JssError::FileOperation(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn from_api_response(status: StatusCode, body: &str) -> Self {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
            let exc_type = val["exc_type"]
                .as_str()
                .unwrap_or("UnknownError")
                .to_string();
            let message = val["exc"]
                .as_str()
                .or_else(|| val["_server_messages"].as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return JssError::ApiError {
                exc_type,
                message,
                status,
            };
        }

        JssError::Http {
            status,
            body: body.to_string(),
        }
    }
}
