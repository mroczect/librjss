use crate::handler::error::JuraganError;
use secrecy::ExposeSecret;
use url::Url;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub base_url: Url,
    pub auth_mode: AuthMode,
    pub expected_sitename: Option<String>,
    pub required_roles: Vec<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub user_agent: String,
    pub insecure_ssl: bool,
}

#[derive(Debug, Clone)]
pub enum AuthMode {
    Session {
        email: secrecy::SecretString,
        password: secrecy::SecretString,
    },
    Token {
        api_key: String,
        api_secret: secrecy::SecretString,
    },
}

impl ClientConfig {
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
