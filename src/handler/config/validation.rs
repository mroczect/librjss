use crate::handler::config::auth_mode::AuthMode;
use crate::handler::config::client_config::ClientConfig;
use crate::handler::error::JssError;
use secrecy::ExposeSecret;

impl ClientConfig {
    pub fn validate(&self) -> Result<(), JssError> {
        if self.base_url.scheme() != "https" && !self.insecure_ssl {
            return Err(JssError::Config(
                "HTTPS required unless insecure_ssl=true".into(),
            ));
        }
        if let AuthMode::Session { email, password } = &self.auth_mode {
            if email.expose_secret().is_empty() || password.expose_secret().is_empty() {
                return Err(JssError::Validation(
                    "Email and password must not be empty".into(),
                ));
            }
        }
        Ok(())
    }
}
