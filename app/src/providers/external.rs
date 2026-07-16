use async_trait::async_trait;
use librjss::handler::error::AuthError;
use librjss::handler::types::{Credentials, UserInfo, UserProvider};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct ExternalSessionStore {
    map: Mutex<HashMap<String, String>>,
}

impl ExternalSessionStore {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn set(&self, user_id: String, cookie: String) {
        self.map.lock().unwrap().insert(user_id, cookie);
    }

    pub fn get(&self, user_id: &str) -> Option<String> {
        self.map.lock().unwrap().get(user_id).cloned()
    }

    pub fn remove(&self, user_id: &str) {
        self.map.lock().unwrap().remove(user_id);
    }
}

pub struct ExternalUserProvider {
    pub client: reqwest::Client,
    pub session_store: std::sync::Arc<ExternalSessionStore>,
}

#[async_trait]
impl UserProvider for ExternalUserProvider {
    async fn authenticate(&self, creds: &Credentials) -> Result<UserInfo, AuthError> {
        let res = self
            .client
            .post("https://app.juragansejati.biz.id/login")
            .json(creds)
            .send()
            .await
            .map_err(|e| AuthError::Internal(format!("External request failed: {}", e)))?;

        if !res.status().is_success() {
            return Err(AuthError::InvalidCredentials);
        }

        let external_cookie = res
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("; ");

        let user_id = res
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|j| j["user_id"].as_str().map(String::from))
            .unwrap_or_else(|| creds.username.clone());

        self.session_store.set(user_id.clone(), external_cookie);

        Ok(UserInfo {
            user_id,
            extra: serde_json::json!({"external_authenticated": true}),
        })
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError> {
        Ok(self.session_store.get(user_id).map(|_| UserInfo {
            user_id: user_id.to_string(),
            extra: serde_json::json!({}),
        }))
    }
}
