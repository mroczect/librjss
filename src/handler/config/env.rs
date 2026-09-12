use crate::handler::config::{AuthMode, ClientConfig};
use crate::handler::error::JssError;
use secrecy::SecretString;
use std::env;
use url::Url;

impl ClientConfig {
    pub fn from_env() -> Result<Self, JssError> {
        let base_url_raw = read_env(&["JSS_BASE_URL", "JSS_URL"]).ok_or_else(|| {
            JssError::Config("JSS_BASE_URL (atau JSS_URL) belum di-set di environment".into())
        })?;

        let base_url = Url::parse(base_url_raw.trim_end_matches('/'))
            .map_err(|e| JssError::Config(format!("JSS_BASE_URL tidak valid: {e}")))?;

        let auth_mode = if let Some(api_key) = read_env(&["JSS_TOKEN_KEY"]) {
            let api_secret = read_env(&["JSS_TOKEN_SECRET"]).ok_or_else(|| {
                JssError::Config("JSS_TOKEN_KEY di-set tapi JSS_TOKEN_SECRET tidak ada".into())
            })?;
            AuthMode::Token {
                api_key,
                api_secret: SecretString::new(api_secret.into_boxed_str()),
            }
        } else {
            let email = read_env(&["JSS_EMAIL", "JSS_USR"]).ok_or_else(|| {
                JssError::Config(
                    "JSS_EMAIL (atau JSS_USR) belum di-set, dan JSS_TOKEN_KEY juga tidak ada"
                        .into(),
                )
            })?;
            let password = read_env(&["JSS_PASSWORD", "JSS_PWD"]).ok_or_else(|| {
                JssError::Config("JSS_PASSWORD (atau JSS_PWD) belum di-set".into())
            })?;
            AuthMode::Session {
                email: SecretString::new(email.into_boxed_str()),
                password: SecretString::new(password.into_boxed_str()),
            }
        };

        let required_roles = read_env(&["JSS_REQUIRED_ROLES"])
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let timeout_secs = read_env(&["JSS_TIMEOUT_SECS"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        let max_retries = read_env(&["JSS_MAX_RETRIES"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let user_agent = read_env(&["JSS_USER_AGENT"]).unwrap_or_else(|| "librjss/2.3.0".into());

        let insecure_ssl = read_env(&["JSS_INSECURE_SSL"])
            .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        let readonly_guard = read_env(&["JSS_READONLY_GUARD"])
            .map(|s| !matches!(s.to_ascii_lowercase().as_str(), "false" | "0" | "no"))
            .unwrap_or(true);

        Ok(ClientConfig {
            base_url,
            auth_mode,
            expected_sitename: read_env(&["JSS_EXPECTED_SITENAME"]),
            required_roles,
            timeout_secs,
            max_retries,
            user_agent,
            insecure_ssl,
            readonly_guard,
        })
    }
}

fn read_env(keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Ok(v) = env::var(k)
            && !v.is_empty()
        {
            return Some(v);
        }
    }
    None
}
