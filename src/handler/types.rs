use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

use crate::error::AuthError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: String) -> Self {
        SessionId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub user_id: String,
    pub data: serde_json::Value,
    pub issued_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub idle_deadline: Option<OffsetDateTime>,
}

#[async_trait]
pub trait UserProvider: Send + Sync {
    async fn authenticate(&self, credentials: &Credentials) -> Result<UserInfo, AuthError>;
    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError>;
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, id: &SessionId, info: &SessionInfo) -> Result<(), AuthError>;
    async fn load(&self, id: &SessionId) -> Result<Option<SessionInfo>, AuthError>;
    async fn delete(&self, id: &SessionId) -> Result<(), AuthError>;
    async fn cleanup(&self) -> Result<(), AuthError>;
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: http::StatusCode,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status: http::StatusCode, body: String) -> Self {
        HttpResponse {
            status,
            headers: vec![],
            body,
        }
    }

    pub fn json(status: http::StatusCode, body: serde_json::Value) -> Self {
        HttpResponse {
            status,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: body.to_string(),
        }
    }

    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.push((key, value));
        self
    }
}
