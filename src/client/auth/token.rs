use crate::handler::types::session::SessionInfo;
use secrecy::SecretString;
use tracing::info;

pub(crate) fn setup_token_session(expected_sitename: Option<String>) -> SessionInfo {
    info!("Using token API key");
    SessionInfo {
        sid: SecretString::new(Box::from("token-mode")),
        csrf_token: SecretString::new(Box::from("")),
        full_name: None,
        sitename: expected_sitename.unwrap_or_default(),
        roles: vec![],
    }
}
