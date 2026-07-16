use crate::error::AuthError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Default for SameSite {
    fn default() -> Self {
        SameSite::Lax
    }
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub cookie_name: String,
    pub cookie_path: String,
    pub cookie_domain: Option<String>,
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
    pub cookie_same_site: SameSite,
    pub session_lifetime: time::Duration,
    pub session_idle_timeout: Option<time::Duration>,
    pub login_url: Option<String>,
    pub logout_redirect_url: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            cookie_name: "sid".to_string(),
            cookie_path: "/".to_string(),
            cookie_domain: None,
            cookie_secure: true,
            cookie_http_only: true,
            cookie_same_site: SameSite::default(),
            session_lifetime: time::Duration::hours(24),
            session_idle_timeout: None,
            login_url: None,
            logout_redirect_url: None,
        }
    }
}

impl AuthConfig {
    pub fn builder() -> AuthConfigBuilder {
        AuthConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct AuthConfigBuilder {
    config: AuthConfig,
}

impl AuthConfigBuilder {
    pub fn cookie_name(mut self, name: String) -> Self {
        self.config.cookie_name = name;
        self
    }

    pub fn cookie_path(mut self, path: String) -> Self {
        self.config.cookie_path = path;
        self
    }

    pub fn cookie_domain(mut self, domain: String) -> Self {
        self.config.cookie_domain = Some(domain);
        self
    }

    pub fn cookie_secure(mut self, secure: bool) -> Self {
        self.config.cookie_secure = secure;
        self
    }

    pub fn cookie_http_only(mut self, http_only: bool) -> Self {
        self.config.cookie_http_only = http_only;
        self
    }

    pub fn cookie_same_site(mut self, same_site: SameSite) -> Self {
        self.config.cookie_same_site = same_site;
        self
    }

    pub fn session_lifetime(mut self, lifetime: time::Duration) -> Self {
        self.config.session_lifetime = lifetime;
        self
    }

    pub fn session_idle_timeout(mut self, timeout: time::Duration) -> Self {
        self.config.session_idle_timeout = Some(timeout);
        self
    }

    pub fn login_url(mut self, url: String) -> Self {
        self.config.login_url = Some(url);
        self
    }

    pub fn logout_redirect_url(mut self, url: String) -> Self {
        self.config.logout_redirect_url = Some(url);
        self
    }

    pub fn build(self) -> Result<AuthConfig, AuthError> {
        if self.config.session_lifetime <= time::Duration::seconds(0) {
            return Err(AuthError::Config(
                "session_lifetime must be positive".into(),
            ));
        }
        if self.config.cookie_name.is_empty() {
            return Err(AuthError::Config("cookie_name cannot be empty".into()));
        }
        Ok(self.config)
    }
}
