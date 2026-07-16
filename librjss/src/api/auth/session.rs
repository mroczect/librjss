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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::config::AuthConfig;
    use crate::handler::types::{SessionId, SessionInfo, SessionStore, UserInfo};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use time::{Duration, OffsetDateTime};

    struct MockStore {
        save: Mutex<Box<dyn FnMut(&SessionId, &SessionInfo) -> Result<(), AuthError> + Send>>,
        load: Mutex<Box<dyn FnMut(&SessionId) -> Result<Option<SessionInfo>, AuthError> + Send>>,
        delete: Mutex<Box<dyn FnMut(&SessionId) -> Result<(), AuthError> + Send>>,
    }

    impl MockStore {
        fn new(
            save: impl FnMut(&SessionId, &SessionInfo) -> Result<(), AuthError> + Send + 'static,
            load: impl FnMut(&SessionId) -> Result<Option<SessionInfo>, AuthError> + Send + 'static,
            delete: impl FnMut(&SessionId) -> Result<(), AuthError> + Send + 'static,
        ) -> Self {
            Self {
                save: Mutex::new(Box::new(save)),
                load: Mutex::new(Box::new(load)),
                delete: Mutex::new(Box::new(delete)),
            }
        }
    }

    #[async_trait]
    impl SessionStore for MockStore {
        async fn save(&self, id: &SessionId, info: &SessionInfo) -> Result<(), AuthError> {
            (self.save.lock().unwrap())(id, info)
        }
        async fn load(&self, id: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
            (self.load.lock().unwrap())(id)
        }
        async fn delete(&self, id: &SessionId) -> Result<(), AuthError> {
            (self.delete.lock().unwrap())(id)
        }
        async fn cleanup(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }

    fn test_config() -> AuthConfig {
        AuthConfig::default()
    }

    fn test_user_info() -> UserInfo {
        UserInfo {
            user_id: "user1".into(),
            extra: serde_json::json!({"role":"admin"}),
        }
    }

    #[test]
    fn test_generate_session_id_length_and_uniqueness() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        for _ in 0..100 {
            let id = generate_session_id();
            assert_eq!(id.as_str().len(), 32);
            assert!(id.as_str().chars().all(|c| c.is_ascii_alphanumeric()));
            set.insert(id.to_string());
        }
        assert_eq!(set.len(), 100);
    }

    #[tokio::test]
    async fn test_create_session_success() {
        let saved_id = Arc::new(Mutex::new(None));
        let saved_info = Arc::new(Mutex::new(None));
        let sid_clone = saved_id.clone();
        let sinfo_clone = saved_info.clone();

        let store = MockStore::new(
            move |id, info| {
                *sid_clone.lock().unwrap() = Some(id.clone());
                *sinfo_clone.lock().unwrap() = Some(info.clone());
                Ok(())
            },
            |_| Ok(None),
            |_| Ok(()),
        );

        let (id, info) = create_session(&store, &test_config(), &test_user_info())
            .await
            .unwrap();
        assert_eq!(id, saved_id.lock().unwrap().as_ref().unwrap().clone());
        assert_eq!(info.user_id, "user1");
        assert_eq!(info.data, serde_json::json!({"role":"admin"}));
        assert!(info.issued_at <= OffsetDateTime::now_utc());
        assert!(info.expires_at > info.issued_at);
        assert!(info.idle_deadline.is_none());
        let expected_expiry = info.issued_at + test_config().session_lifetime;
        assert!((info.expires_at - expected_expiry).whole_seconds().abs() <= 1);
    }

    #[tokio::test]
    async fn test_create_session_with_idle_timeout() {
        let mut cfg = test_config();
        cfg.session_idle_timeout = Some(Duration::minutes(15));
        let called = Arc::new(Mutex::new(false));
        let c = called.clone();
        let store = MockStore::new(
            move |_, info| {
                assert!(info.idle_deadline.is_some());
                let expected_idle = info.issued_at + Duration::minutes(15);
                assert!(
                    (info.idle_deadline.unwrap() - expected_idle)
                        .whole_seconds()
                        .abs()
                        <= 1
                );
                *c.lock().unwrap() = true;
                Ok(())
            },
            |_| Ok(None),
            |_| Ok(()),
        );
        create_session(&store, &cfg, &test_user_info())
            .await
            .unwrap();
        assert!(*called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_create_session_store_error() {
        let store = MockStore::new(
            |_, _| Err(AuthError::Internal("db error".into())),
            |_| Ok(None),
            |_| Ok(()),
        );
        let err = create_session(&store, &test_config(), &test_user_info())
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Internal(m) if m.contains("db error")));
    }

    #[tokio::test]
    async fn test_validate_session_valid() {
        let info = SessionInfo {
            user_id: "user123".into(),
            data: serde_json::json!({}),
            issued_at: OffsetDateTime::now_utc() - Duration::minutes(5),
            expires_at: OffsetDateTime::now_utc() + Duration::hours(1),
            idle_deadline: None,
        };
        let store = MockStore::new(|_, _| Ok(()), move |_| Ok(Some(info.clone())), |_| Ok(()));
        let result = validate_session(&store, &test_config(), &SessionId::new("valid".into()))
            .await
            .unwrap();
        assert_eq!(result.user_id, "user123");
    }

    #[tokio::test]
    async fn test_validate_session_expired_by_lifetime() {
        let expired = SessionInfo {
            user_id: "u".into(),
            data: serde_json::json!({}),
            issued_at: OffsetDateTime::now_utc() - Duration::hours(2),
            expires_at: OffsetDateTime::now_utc() - Duration::hours(1),
            idle_deadline: None,
        };
        let delete_called = Arc::new(Mutex::new(false));
        let dc = delete_called.clone();
        let store = MockStore::new(
            |_, _| Ok(()),
            move |_| Ok(Some(expired.clone())),
            move |_| {
                *dc.lock().unwrap() = true;
                Ok(())
            },
        );
        let err = validate_session(&store, &test_config(), &SessionId::new("exp".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::SessionExpired));
        assert!(*delete_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_validate_session_expired_by_idle() {
        let info = SessionInfo {
            user_id: "u".into(),
            data: serde_json::json!({}),
            issued_at: OffsetDateTime::now_utc() - Duration::hours(1),
            expires_at: OffsetDateTime::now_utc() + Duration::hours(1),
            idle_deadline: Some(OffsetDateTime::now_utc() - Duration::minutes(1)),
        };
        let delete_called = Arc::new(Mutex::new(false));
        let dc = delete_called.clone();
        let store = MockStore::new(
            |_, _| Ok(()),
            move |_| Ok(Some(info.clone())),
            move |_| {
                *dc.lock().unwrap() = true;
                Ok(())
            },
        );
        let err = validate_session(&store, &test_config(), &SessionId::new("idle".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::SessionExpired));
        assert!(*delete_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_validate_session_not_found() {
        let store = MockStore::new(|_, _| Ok(()), |_| Ok(None), |_| Ok(()));
        let err = validate_session(&store, &test_config(), &SessionId::new("ghost".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::SessionNotFound));
    }

    #[tokio::test]
    async fn test_validate_session_load_error() {
        let store = MockStore::new(
            |_, _| Ok(()),
            |_| Err(AuthError::Internal("load fail".into())),
            |_| Ok(()),
        );
        let err = validate_session(&store, &test_config(), &SessionId::new("err".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Internal(msg) if msg == "load fail"));
    }

    #[tokio::test]
    async fn test_destroy_session_success() {
        let deleted_id = Arc::new(Mutex::new(None));
        let did = deleted_id.clone();
        let store = MockStore::new(
            |_, _| Ok(()),
            |_| Ok(None),
            move |id| {
                *did.lock().unwrap() = Some(id.clone());
                Ok(())
            },
        );
        let sid = SessionId::new("kill".into());
        destroy_session(&store, &sid).await.unwrap();
        assert_eq!(*deleted_id.lock().unwrap(), Some(sid));
    }

    #[tokio::test]
    async fn test_destroy_session_error() {
        let store = MockStore::new(
            |_, _| Ok(()),
            |_| Ok(None),
            |_| Err(AuthError::Internal("del fail".into())),
        );
        let err = destroy_session(&store, &SessionId::new("bad".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }
}
