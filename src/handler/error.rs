use http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AuthError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("account locked until {until}")]
    AccountLocked { until: String },

    #[error("session expired")]
    SessionExpired,

    #[error("session not found")]
    SessionNotFound,

    #[error("internal server error")]
    Internal(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl AuthError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::Config(_) | AuthError::Internal(_) | AuthError::Serialization(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthError::AccountLocked { .. } => StatusCode::FORBIDDEN,
            AuthError::SessionExpired | AuthError::SessionNotFound => StatusCode::UNAUTHORIZED,
        }
    }

    pub fn to_json_body(&self) -> serde_json::Value {
        serde_json::json!({
            "error": self.to_string()
        })
    }
}
