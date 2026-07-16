use rand::Rng;
use time::OffsetDateTime;

use crate::error::AuthError;
use crate::handler::types::{SessionId, SessionInfo, SessionStore};

pub(crate) fn generate_session_id() -> SessionId {
    let id: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    SessionId::new(id)
}

pub(crate) async fn create_session(
    store: &dyn SessionStore,
    config: &crate::handler::config::AuthConfig,
    user_info: &crate::handler::types::UserInfo,
) -> Result<(SessionId, SessionInfo), AuthError> {
    let id = generate_session_id();
    let now = OffsetDateTime::now_utc();
    let expires = now + config.session_lifetime;
    let idle_deadline = config.session_idle_timeout.map(|to| now + to);

    let info = SessionInfo {
        user_id: user_info.user_id.clone(),
        data: user_info.extra.clone(),
        issued_at: now,
        expires_at: expires,
        idle_deadline,
    };

    store.save(&id, &info).await?;
    Ok((id, info))
}

pub(crate) async fn validate_session(
    store: &dyn SessionStore,
    _config: &crate::handler::config::AuthConfig,
    id: &SessionId,
) -> Result<SessionInfo, AuthError> {
    let info = store.load(id).await?.ok_or(AuthError::SessionNotFound)?;

    let now = OffsetDateTime::now_utc();
    if now > info.expires_at {
        store.delete(id).await?;
        return Err(AuthError::SessionExpired);
    }

    if let Some(idle_deadline) = info.idle_deadline {
        if now > idle_deadline {
            store.delete(id).await?;
            return Err(AuthError::SessionExpired);
        }
    }

    Ok(info)
}

pub(crate) async fn destroy_session(
    store: &dyn SessionStore,
    id: &SessionId,
) -> Result<(), AuthError> {
    store.delete(id).await
}
