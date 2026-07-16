use async_trait::async_trait;
use librjss::api::auth::session;
use librjss::handler::config::AuthConfig;
use librjss::handler::error::AuthError;
use librjss::handler::types::{SessionId, SessionInfo, SessionStore, UserInfo};
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

struct MockSessionStore {
    save_fn: Arc<dyn Fn(&SessionId, &SessionInfo) -> Result<(), AuthError> + Send + Sync>,
    load_fn: Arc<dyn Fn(&SessionId) -> Result<Option<SessionInfo>, AuthError> + Send + Sync>,
    delete_fn: Arc<dyn Fn(&SessionId) -> Result<(), AuthError> + Send + Sync>,
}

impl MockSessionStore {
    fn new(
        save_fn: impl Fn(&SessionId, &SessionInfo) -> Result<(), AuthError> + Send + Sync + 'static,
        load_fn: impl Fn(&SessionId) -> Result<Option<SessionInfo>, AuthError> + Send + Sync + 'static,
        delete_fn: impl Fn(&SessionId) -> Result<(), AuthError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            save_fn: Arc::new(save_fn),
            load_fn: Arc::new(load_fn),
            delete_fn: Arc::new(delete_fn),
        }
    }
}

#[async_trait]
impl SessionStore for MockSessionStore {
    async fn save(&self, id: &SessionId, info: &SessionInfo) -> Result<(), AuthError> {
        (self.save_fn)(id, info)
    }
    async fn load(&self, id: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
        (self.load_fn)(id)
    }
    async fn delete(&self, id: &SessionId) -> Result<(), AuthError> {
        (self.delete_fn)(id)
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

#[tokio::test]
async fn test_generate_session_id_length_and_uniqueness() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    for _ in 0..100 {
        let id = session::generate_session_id();
        assert_eq!(id.as_str().len(), 32);
        assert!(id.as_str().chars().all(|c| c.is_ascii_alphanumeric()));
        set.insert(id.to_string());
    }
    assert_eq!(set.len(), 100);
}

#[tokio::test]
async fn test_create_session_success() {
    let mut saved_id = None;
    let mut saved_info = None;
    let store = MockSessionStore::new(
        |id, info| {
            saved_id = Some(id.clone());
            saved_info = Some(info.clone());
            Ok(())
        },
        |_| Ok(None),
        |_| Ok(()),
    );
    let (id, info) = session::create_session(&store, &test_config(), &test_user_info())
        .await
        .unwrap();
    assert_eq!(id, saved_id.unwrap());
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
    let store = MockSessionStore::new(
        |_, info| {
            assert!(info.idle_deadline.is_some());
            let expected_idle = info.issued_at + Duration::minutes(15);
            assert!(
                (info.idle_deadline.unwrap() - expected_idle)
                    .whole_seconds()
                    .abs()
                    <= 1
            );
            Ok(())
        },
        |_| Ok(None),
        |_| Ok(()),
    );
    session::create_session(&store, &cfg, &test_user_info())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_create_session_store_error() {
    let store = MockSessionStore::new(
        |_, _| Err(AuthError::Internal("db error".into())),
        |_| Ok(None),
        |_| Ok(()),
    );
    let err = session::create_session(&store, &test_config(), &test_user_info())
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
    let store = MockSessionStore::new(|_, _| Ok(()), move |_| Ok(Some(info.clone())), |_| Ok(()));
    let result = session::validate_session(&store, &test_config(), &SessionId::new("valid".into()))
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
    let mut delete_called = false;
    let store = MockSessionStore::new(
        |_, _| Ok(()),
        move |_| Ok(Some(expired.clone())),
        move |_| {
            delete_called = true;
            Ok(())
        },
    );
    let err = session::validate_session(&store, &test_config(), &SessionId::new("exp".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::SessionExpired));
    assert!(delete_called);
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
    let mut delete_called = false;
    let store = MockSessionStore::new(
        |_, _| Ok(()),
        move |_| Ok(Some(info.clone())),
        move |_| {
            delete_called = true;
            Ok(())
        },
    );
    let err = session::validate_session(&store, &test_config(), &SessionId::new("idle".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::SessionExpired));
    assert!(delete_called);
}

#[tokio::test]
async fn test_validate_session_not_found() {
    let store = MockSessionStore::new(|_, _| Ok(()), |_| Ok(None), |_| Ok(()));
    let err = session::validate_session(&store, &test_config(), &SessionId::new("ghost".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::SessionNotFound));
}

#[tokio::test]
async fn test_validate_session_load_error() {
    let store = MockSessionStore::new(
        |_, _| Ok(()),
        |_| Err(AuthError::Internal("load fail".into())),
        |_| Ok(()),
    );
    let err = session::validate_session(&store, &test_config(), &SessionId::new("err".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Internal(msg) if msg == "load fail"));
}

#[tokio::test]
async fn test_destroy_session_success() {
    let mut deleted_id = None;
    let store = MockSessionStore::new(
        |_, _| Ok(()),
        |_| Ok(None),
        move |id| {
            deleted_id = Some(id.clone());
            Ok(())
        },
    );
    let sid = SessionId::new("kill".into());
    session::destroy_session(&store, &sid).await.unwrap();
    assert_eq!(deleted_id.unwrap(), sid);
}

#[tokio::test]
async fn test_destroy_session_error() {
    let store = MockSessionStore::new(
        |_, _| Ok(()),
        |_| Ok(None),
        |_| Err(AuthError::Internal("del fail".into())),
    );
    let err = session::destroy_session(&store, &SessionId::new("bad".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Internal(_)));
}
