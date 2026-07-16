use crate::providers::external::ExternalSessionStore;
use librjss::api::AuthManager;
use std::sync::Arc;

pub struct AppState {
    pub auth_manager: AuthManager,
    pub external_session_store: Option<Arc<ExternalSessionStore>>,
    pub http_client: reqwest::Client,
}
