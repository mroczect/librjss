use crate::config::{AuthConfig, SameSite};
use crate::error::AuthError;
use std::env;

impl AuthConfig {
    pub fn from_env() -> Result<Self, AuthError> {
        let mut builder = AuthConfig::builder();

        if let Ok(v) = env::var("AUTH_COOKIE_NAME") {
            builder = builder.cookie_name(v);
        }
        if let Ok(v) = env::var("AUTH_COOKIE_PATH") {
            builder = builder.cookie_path(v);
        }
        if let Ok(v) = env::var("AUTH_COOKIE_DOMAIN") {
            builder = builder.cookie_domain(v);
        }
        if let Ok(v) = env::var("AUTH_COOKIE_SECURE") {
            let secure = v.parse::<bool>().map_err(|_| {
                AuthError::Config("AUTH_COOKIE_SECURE must be true or false".into())
            })?;
            builder = builder.cookie_secure(secure);
        }
        if let Ok(v) = env::var("AUTH_COOKIE_HTTP_ONLY") {
            let http_only = v.parse::<bool>().map_err(|_| {
                AuthError::Config("AUTH_COOKIE_HTTP_ONLY must be true or false".into())
            })?;
            builder = builder.cookie_http_only(http_only);
        }
        if let Ok(v) = env::var("AUTH_COOKIE_SAME_SITE") {
            let same_site = match v.to_lowercase().as_str() {
                "strict" => SameSite::Strict,
                "lax" => SameSite::Lax,
                "none" => SameSite::None,
                _ => {
                    return Err(AuthError::Config(
                        "AUTH_COOKIE_SAME_SITE must be strict, lax, or none".into(),
                    ));
                }
            };
            builder = builder.cookie_same_site(same_site);
        }
        if let Ok(v) = env::var("AUTH_SESSION_LIFETIME") {
            let secs = v.parse::<i64>().map_err(|_| {
                AuthError::Config("AUTH_SESSION_LIFETIME must be an integer (seconds)".into())
            })?;
            if secs <= 0 {
                return Err(AuthError::Config(
                    "AUTH_SESSION_LIFETIME must be positive".into(),
                ));
            }
            builder = builder.session_lifetime(time::Duration::seconds(secs));
        }
        if let Ok(v) = env::var("AUTH_SESSION_IDLE_TIMEOUT") {
            let secs = v.parse::<i64>().map_err(|_| {
                AuthError::Config("AUTH_SESSION_IDLE_TIMEOUT must be an integer (seconds)".into())
            })?;
            if secs <= 0 {
                return Err(AuthError::Config(
                    "AUTH_SESSION_IDLE_TIMEOUT must be positive".into(),
                ));
            }
            builder = builder.session_idle_timeout(time::Duration::seconds(secs));
        }
        if let Ok(v) = env::var("AUTH_LOGIN_URL") {
            builder = builder.login_url(v);
        }
        if let Ok(v) = env::var("AUTH_LOGOUT_REDIRECT_URL") {
            builder = builder.logout_redirect_url(v);
        }

        builder.build()
    }
}
