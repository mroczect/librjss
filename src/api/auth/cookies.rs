use cookie::{Cookie, SameSite};
use time::Duration;

use crate::handler::config::AuthConfig;
use crate::handler::types::SessionId;

pub fn create_session_cookie(session_id: &SessionId, config: &AuthConfig) -> Cookie<'static> {
    let max_age = config.session_lifetime;
    let mut cookie = Cookie::build((config.cookie_name.clone(), session_id.to_string()))
        .path(config.cookie_path.clone())
        .http_only(config.cookie_http_only)
        .secure(config.cookie_secure)
        .same_site(match config.cookie_same_site {
            crate::handler::config::SameSite::Strict => SameSite::Strict,
            crate::handler::config::SameSite::Lax => SameSite::Lax,
            crate::handler::config::SameSite::None => SameSite::None,
        })
        .max_age(max_age)
        .finish();

    if let Some(ref domain) = config.cookie_domain {
        cookie.set_domain(domain.clone());
    }

    cookie
}

pub fn parse_session_id_from_cookie(cookie_header: &str, config: &AuthConfig) -> Option<SessionId> {
    let cookies = cookie_header.split("; ");
    for c in cookies {
        if let Some((key, value)) = c.split_once('=') {
            if key.trim() == config.cookie_name {
                return Some(SessionId::new(value.trim().to_string()));
            }
        }
    }
    None
}

pub fn create_removal_cookie(config: &AuthConfig) -> Cookie<'static> {
    Cookie::build((config.cookie_name.clone(), ""))
        .path(config.cookie_path.clone())
        .http_only(config.cookie_http_only)
        .secure(config.cookie_secure)
        .same_site(match config.cookie_same_site {
            crate::handler::config::SameSite::Strict => SameSite::Strict,
            crate::handler::config::SameSite::Lax => SameSite::Lax,
            crate::handler::config::SameSite::None => SameSite::None,
        })
        .max_age(Duration::seconds(0))
        .finish()
}
