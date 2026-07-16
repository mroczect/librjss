use async_trait::async_trait;
use librjss::api::AuthManager;
use librjss::api::auth::cookies;
use librjss::handler::config::AuthConfig;
use librjss::handler::error::AuthError;
use librjss::handler::types::{
    Credentials, SessionId, SessionInfo, SessionStore, UserInfo, UserProvider,
};
use std::sync::Arc;
use time::Duration;

struct MockUserProvider {
    result: Result<UserInfo, AuthError>,
}
#[async_trait]
impl UserProvider for MockUserProvider {
    async fn authenticate(&self, _: &Credentials) -> Result<UserInfo, AuthError> {
        self.result.clone()
    }
    async fn get_user_by_id(&self, _: &str) -> Result<Option<UserInfo>, AuthError> {
        Ok(None)
    }
}

struct DummySessionStore;
#[async_trait]
impl SessionStore for DummySessionStore {
    async fn save(&self, _: &SessionId, _: &SessionInfo) -> Result<(), AuthError> {
        Ok(())
    }
    async fn load(&self, _: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
        Ok(None)
    }
    async fn delete(&self, _: &SessionId) -> Result<(), AuthError> {
        Ok(())
    }
    async fn cleanup(&self) -> Result<(), AuthError> {
        Ok(())
    }
}

fn test_config() -> AuthConfig {
    AuthConfig::builder()
        .cookie_name("sid".into())
        .session_lifetime(Duration::hours(1))
        .build()
        .unwrap()
}

fn test_user_info() -> UserInfo {
    UserInfo {
        user_id: "user123".into(),
        extra: serde_json::json!({}),
    }
}

#[tokio::test]
async fn test_login_empty_username() {
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(DummySessionStore),
    );
    let err = mgr
        .login(Credentials {
            username: "  ".into(),
            password: "p".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn test_login_empty_password() {
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(DummySessionStore),
    );
    let err = mgr
        .login(Credentials {
            username: "u".into(),
            password: "".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn test_login_user_provider_error() {
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Err(AuthError::InvalidCredentials),
        }),
        Arc::new(DummySessionStore),
    );
    let err = mgr
        .login(Credentials {
            username: "u".into(),
            password: "p".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn test_login_session_creation_error() {
    struct FailingStore;
    #[async_trait]
    impl SessionStore for FailingStore {
        async fn save(&self, _: &SessionId, _: &SessionInfo) -> Result<(), AuthError> {
            Err(AuthError::Internal("db save fail".into()))
        }
        async fn load(&self, _: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
            Ok(None)
        }
        async fn delete(&self, _: &SessionId) -> Result<(), AuthError> {
            Ok(())
        }
        async fn cleanup(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(FailingStore),
    );
    let err = mgr
        .login(Credentials {
            username: "u".into(),
            password: "p".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Internal(m) if m.contains("session creation failed")));
}

#[tokio::test]
async fn test_login_success_response() {
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(DummySessionStore),
    );
    let resp = mgr
        .login(Credentials {
            username: "u".into(),
            password: "p".into(),
        })
        .await
        .unwrap();
    assert_eq!(resp.status, http::StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["user_id"], "user123");
    assert!(
        resp.headers
            .iter()
            .any(|(k, v)| k == "Set-Cookie" && v.starts_with("sid="))
    );
}

#[tokio::test]
async fn test_logout_with_session_id() {
    struct SpyStore {
        deleted: std::sync::Mutex<Vec<SessionId>>,
    }
    #[async_trait]
    impl SessionStore for SpyStore {
        async fn save(&self, _: &SessionId, _: &SessionInfo) -> Result<(), AuthError> {
            Ok(())
        }
        async fn load(&self, _: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
            Ok(None)
        }
        async fn delete(&self, id: &SessionId) -> Result<(), AuthError> {
            self.deleted.lock().unwrap().push(id.clone());
            Ok(())
        }
        async fn cleanup(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }
    let store = Arc::new(SpyStore {
        deleted: std::sync::Mutex::new(vec![]),
    });
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        store.clone(),
    );
    let sid = SessionId::new("session1".into());
    let resp = mgr.logout(Some(&sid)).await.unwrap();
    assert_eq!(resp.status, http::StatusCode::OK);
    assert!(resp.body.contains("logged_out"));
    assert!(resp.headers.iter().any(|(_, v)| v.contains("sid=;")));
    assert_eq!(store.deleted.lock().unwrap().first().unwrap(), &sid);
}

#[tokio::test]
async fn test_logout_without_session_id_no_delete() {
    struct SpyStore {
        deleted: std::sync::Mutex<bool>,
    }
    #[async_trait]
    impl SessionStore for SpyStore {
        async fn save(&self, _: &SessionId, _: &SessionInfo) -> Result<(), AuthError> {
            Ok(())
        }
        async fn load(&self, _: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
            Ok(None)
        }
        async fn delete(&self, _: &SessionId) -> Result<(), AuthError> {
            *self.deleted.lock().unwrap() = true;
            Ok(())
        }
        async fn cleanup(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }
    let store = Arc::new(SpyStore {
        deleted: std::sync::Mutex::new(false),
    });
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        store.clone(),
    );
    let resp = mgr.logout(None).await.unwrap();
    assert!(!*store.deleted.lock().unwrap());
    assert!(resp.headers.iter().any(|(_, v)| v.contains("sid=;")));
}

#[tokio::test]
async fn test_logout_with_redirect() {
    let mut cfg = test_config();
    cfg.logout_redirect_url = Some("/bye".into());
    let mgr = AuthManager::new(
        cfg,
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(DummySessionStore),
    );
    let resp = mgr.logout(Some(&SessionId::new("x".into()))).await.unwrap();
    assert_eq!(resp.status, http::StatusCode::FOUND);
    assert!(
        resp.headers
            .iter()
            .any(|(k, v)| k == "Location" && v == "/bye")
    );
}

#[tokio::test]
async fn test_logout_destroy_error_still_succeeds() {
    struct FailDeleteStore;
    #[async_trait]
    impl SessionStore for FailDeleteStore {
        async fn save(&self, _: &SessionId, _: &SessionInfo) -> Result<(), AuthError> {
            Ok(())
        }
        async fn load(&self, _: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
            Ok(None)
        }
        async fn delete(&self, _: &SessionId) -> Result<(), AuthError> {
            Err(AuthError::Internal("no".into()))
        }
        async fn cleanup(&self) -> Result<(), AuthError> {
            Ok(())
        }
    }
    let mgr = AuthManager::new(
        test_config(),
        Arc::new(MockUserProvider {
            result: Ok(test_user_info()),
        }),
        Arc::new(FailDeleteStore),
    );
    let resp = mgr
        .logout(Some(&SessionId::new("fail".into())))
        .await
        .unwrap();
    assert_eq!(resp.status, http::StatusCode::OK);
}

#[test]
fn test_parse_session_id_found() {
    let cfg = test_config();
    let id = cookies::parse_session_id_from_cookie("sid=abc123; other=val", &cfg).unwrap();
    assert_eq!(id.as_str(), "abc123");
}

#[test]
fn test_parse_session_id_with_whitespace() {
    let cfg = test_config();
    let id = cookies::parse_session_id_from_cookie(" sid = xyz ", &cfg).unwrap();
    assert_eq!(id.as_str(), "xyz");
}

#[test]
fn test_parse_session_id_not_found() {
    let cfg = test_config();
    assert!(cookies::parse_session_id_from_cookie("other=val", &cfg).is_none());
}

#[test]
fn test_parse_session_id_no_equals() {
    let cfg = test_config();
    assert!(cookies::parse_session_id_from_cookie("sid", &cfg).is_none());
}

#[test]
fn test_create_removal_cookie() {
    let cfg = test_config();
    let cookie = cookies::create_removal_cookie(&cfg);
    assert_eq!(cookie.name(), "sid");
    assert_eq!(cookie.value(), "");
    assert_eq!(cookie.max_age(), Some(Duration::seconds(0)));
    assert_eq!(cookie.path().unwrap(), "/");
}

#[test]
fn test_create_session_cookie_domain_and_attributes() {
    let mut cfg = test_config();
    cfg.cookie_domain = Some("example.org".into());
    let cookie = cookies::create_session_cookie(&SessionId::new("id".into()), &cfg);
    assert_eq!(cookie.domain().unwrap(), "example.org");
    assert!(cookie.http_only().unwrap());
    assert!(cookie.secure().unwrap());
}
