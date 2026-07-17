pub mod auth;
pub mod methods;

use reqwest::Client as ReqwestClient;
use reqwest::Url;
use reqwest::cookie::Jar;
use secrecy::SecretString;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::api::auth::AuthEndpoints;
use crate::handler::config::ClientConfig;
use crate::handler::error::JssError;
use crate::handler::types::SessionInfo;

pub struct RjssClient {
    pub(crate) config: ClientConfig,
    pub(crate) http: ReqwestClient,
    pub(crate) session: Option<SessionInfo>,
    pub(crate) trace_id: String,
    pub(crate) credentials: Option<(SecretString, SecretString)>,
}

impl AuthEndpoints for RjssClient {}

impl RjssClient {
    pub fn new(config: ClientConfig) -> Result<Self, JssError> {
        config.validate()?;

        let cookie_jar = Arc::new(Jar::default());
        let http = ReqwestClient::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .danger_accept_invalid_certs(config.insecure_ssl)
            .user_agent(&config.user_agent)
            .build()?;

        let trace_id = Uuid::new_v4().to_string();
        info!(
            trace_id,
            "Client created with base_url: {}", config.base_url
        );

        Ok(RjssClient {
            config,
            http,
            session: None,
            trace_id,
            credentials: None,
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.config.base_url
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn session_info(&self) -> Option<&SessionInfo> {
        self.session.as_ref()
    }

    pub async fn authenticate(&mut self) -> Result<(), JssError> {
        match self.config.auth_mode.clone() {
            crate::handler::config::AuthMode::Session { email, password } => {
                self.credentials = Some((email.clone(), password.clone()));
                auth::login::login_with_credentials(self, email, password).await
            }
            crate::handler::config::AuthMode::Token { .. } => {
                let session =
                    auth::token::setup_token_session(self.config.expected_sitename.clone());
                self.session = Some(session);
                Ok(())
            }
        }
    }

    pub async fn logout(&mut self) -> Result<(), JssError> {
        auth::logout::logout(self).await
    }

    pub async fn ensure_session(&mut self) -> Result<(), JssError> {
        auth::session::ensure_session(self).await
    }

    pub async fn authenticated_get(&self, path: &str) -> Result<String, JssError> {
        methods::get::authenticated_get(self, path).await
    }

    pub async fn authenticated_post(
        &self,
        path: &str,
        body_json: &str,
    ) -> Result<String, JssError> {
        methods::post::authenticated_post(self, path, body_json).await
    }

    pub async fn authenticated_put(
        &self,
        path: &str,
        body_json: &str,
    ) -> Result<String, JssError> {
        methods::put::authenticated_put(self, path, body_json).await
    }

    pub async fn authenticated_delete(&self, path: &str) -> Result<String, JssError> {
        methods::delete::authenticated_delete(self, path).await
    }
}
