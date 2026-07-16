pub mod auth;

use std::sync::Arc;
use tracing::instrument;

use crate::api::auth::{login, logout, session as session_ops};
use crate::error::AuthError;
use crate::handler::config::AuthConfig;
use crate::handler::types::{
    Credentials, HttpResponse, SessionId, SessionInfo, SessionStore, UserProvider,
};

pub struct AuthManager {
    pub config: AuthConfig,
    pub user_provider: Arc<dyn UserProvider>,
    pub session_store: Arc<dyn SessionStore>,
}

impl AuthManager {
    pub fn new(
        config: AuthConfig,
        user_provider: Arc<dyn UserProvider>,
        session_store: Arc<dyn SessionStore>,
    ) -> Self {
        AuthManager {
            config,
            user_provider,
            session_store,
        }
    }

    #[instrument(skip(self, credentials))]
    pub async fn login(&self, credentials: Credentials) -> Result<HttpResponse, AuthError> {
        login::handle_login(self, credentials).await
    }

    #[instrument(skip(self))]
    pub async fn logout(&self, session_id: Option<&SessionId>) -> Result<HttpResponse, AuthError> {
        logout::handle_logout(self, session_id).await
    }

    #[instrument(skip(self))]
    pub async fn validate_session(&self, session_id: &SessionId) -> Result<SessionInfo, AuthError> {
        session_ops::validate_session(&*self.session_store, &self.config, session_id).await
    }

    #[instrument(skip(self, user_info))]
    pub async fn create_session(
        &self,
        user_info: &crate::handler::types::UserInfo,
    ) -> Result<(SessionId, SessionInfo), AuthError> {
        session_ops::create_session(&*self.session_store, &self.config, user_info).await
    }

    #[instrument(skip(self))]
    pub async fn destroy_session(&self, session_id: &SessionId) -> Result<(), AuthError> {
        session_ops::destroy_session(&*self.session_store, session_id).await
    }
}
