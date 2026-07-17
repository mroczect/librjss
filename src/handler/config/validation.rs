use crate::handler::config::auth_mode::AuthMode;
use crate::handler::config::client_config::ClientConfig;
use crate::handler::error::JuraganError;
use secrecy::ExposeSecret;

impl ClientConfig {
    /// Validates the configuration and returns an error if something is wrong.
    pub fn validate(&self) -> Result<(), JuraganError> {
        if self.base_url.scheme() != "https" && !self.insecure_ssl {
            return Err(JuraganError::Config(
                "HTTPS required unless insecure_ssl=true".into(),
            ));
        }
        if let AuthMode::Session { email, password } = &self.auth_mode {
            if email.expose_secret().is_empty() || password.expose_secret().is_empty() {
                return Err(JuraganError::Validation(
                    "Email and password must not be empty".into(),
                ));
            }
        }
        Ok(())
    }
}
