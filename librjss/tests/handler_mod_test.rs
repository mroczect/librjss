use librjss::handler::config::{AuthConfig, SameSite};
use librjss::handler::error::AuthError;
use librjss::handler::session_store::MemorySessionStore;
use librjss::handler::types::{
    Credentials, HttpResponse, SessionId, SessionInfo, SessionStore, UserInfo,
};

#[test]
fn test_all_re_exports_accessible() {
    let _ = AuthConfig::default();
    let _ = SameSite::Lax;
    let _ = AuthError::InvalidCredentials;
    let _ = SessionId::new("test".into());
    let _ = Credentials {
        username: "u".into(),
        password: "p".into(),
    };
    let _ = UserInfo {
        user_id: "1".into(),
        extra: serde_json::json!({}),
    };
    let _ = SessionInfo {
        user_id: "1".into(),
        data: serde_json::json!({}),
        issued_at: time::OffsetDateTime::now_utc(),
        expires_at: time::OffsetDateTime::now_utc(),
        idle_deadline: None,
    };
    let _ = HttpResponse::new(http::StatusCode::OK, "".into());
    let _ = MemorySessionStore::new();
}

#[test]
fn test_memory_session_store_implements_trait() {
    fn _assert_store<S: SessionStore>() {}
    _assert_store::<MemorySessionStore>();
}

#[test]
fn test_auth_error_is_send_sync() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<AuthError>();
}
