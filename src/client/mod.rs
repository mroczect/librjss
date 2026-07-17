pub mod auth;

use reqwest::Client as ReqwestClient;
use reqwest::Url;
use reqwest::cookie::Jar;
use secrecy::SecretString;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::handler::env::ClientConfig;
use crate::handler::error::JuraganError;
use crate::handler::types::SessionInfo;

pub struct RjssClient {
    pub(crate) config: ClientConfig,
    pub(crate) http: ReqwestClient,
    pub(crate) session: Option<SessionInfo>,
    pub(crate) trace_id: String,
    pub(crate) credentials: Option<(SecretString, SecretString)>,
}

impl RjssClient {
    pub fn new(config: ClientConfig) -> Result<Self, JuraganError> {
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
}
