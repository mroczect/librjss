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
    Http {
        status: http::StatusCode,
        body: String,
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

    #[error("Rate limited")]
    RateLimited,

    #[error("Data parsing error: {0}")]
    Parse(String),

    #[error("Token/secret expired")]
    Expired,

    #[error("Operation cancelled")]
    Cancelled,
}
