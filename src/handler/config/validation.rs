use crate::handler::config::auth_mode::AuthMode;
use crate::handler::config::client_config::ClientConfig;
use crate::handler::error::JssError;
use secrecy::ExposeSecret;
use url::Url;

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

        if matches!(self.base_url.scheme(), "data" | "javascript" | "vbscript") {
            return Err(JssError::Config(format!(
                "Unsupported URL scheme: {}",
                self.base_url.scheme()
            )));
        }

        Ok(())
    }

    pub(crate) fn validate_joined_url(&self, joined: &Url) -> Result<(), JssError> {
        if joined.scheme() != self.base_url.scheme()
            || joined.host_str() != self.base_url.host_str()
            || joined.port() != self.base_url.port()
        {
            return Err(JssError::Validation(
                "Path must be relative to base URL".into(),
            ));
        }
        Ok(())
    }
}
