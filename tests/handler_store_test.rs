use librjss::handler::session_store::MemorySessionStore;
use librjss::handler::types::{SessionId, SessionInfo, SessionStore};
use time::{Duration, OffsetDateTime};

fn make_info(expire_offset: Duration) -> SessionInfo {
    let now = OffsetDateTime::now_utc();
    SessionInfo {
        user_id: "testuser".into(),
        data: serde_json::json!({"test":true}),
        issued_at: now,
        expires_at: now + expire_offset,
        idle_deadline: None,
    }
}

#[tokio::test]
async fn test_save_and_load() {
    let store = MemorySessionStore::new();
    let id = SessionId::new("1".into());
    let info = make_info(Duration::hours(1));
    store.save(&id, &info).await.unwrap();
    let loaded = store.load(&id).await.unwrap().unwrap();
    assert_eq!(loaded.user_id, "testuser");
    assert_eq!(loaded.data, serde_json::json!({"test":true}));
}

#[tokio::test]
async fn test_load_missing() {
    let store = MemorySessionStore::new();
    assert!(
        store
            .load(&SessionId::new("nope".into()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn test_delete() {
    let store = MemorySessionStore::new();
    let id = SessionId::new("del".into());
    store
        .save(&id, &make_info(Duration::hours(1)))
        .await
        .unwrap();
    store.delete(&id).await.unwrap();
    assert!(store.load(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_cleanup_removes_expired_only() {
    let store = MemorySessionStore::new();
    let valid_id = SessionId::new("v".into());
    let expired_id = SessionId::new("e".into());
    let valid_info = make_info(Duration::hours(2));
    let mut expired_info = make_info(Duration::seconds(-10));
    expired_info.expires_at = OffsetDateTime::now_utc() - Duration::seconds(1);

    store.save(&valid_id, &valid_info).await.unwrap();
    store.save(&expired_id, &expired_info).await.unwrap();

    store.cleanup().await.unwrap();
    assert!(store.load(&valid_id).await.unwrap().is_some());
    assert!(store.load(&expired_id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_cleanup_when_all_expired() {
    let store = MemorySessionStore::new();
    let id = SessionId::new("old".into());
    let mut info = make_info(Duration::seconds(-5));
    info.expires_at = OffsetDateTime::now_utc() - Duration::seconds(1);
    store.save(&id, &info).await.unwrap();
    store.cleanup().await.unwrap();
    assert!(store.load(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_concurrent_access() {
    use std::sync::Arc;
    use tokio::task;
    let store = Arc::new(MemorySessionStore::new());
    let store_clone = store.clone();
    let id = SessionId::new("concurrent".into());
    let info = make_info(Duration::hours(1));
    store.save(&id, &info).await.unwrap();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let store = store_clone.clone();
            let id = id.clone();
            task::spawn(async move { store.load(&id).await.unwrap().unwrap() })
        })
        .collect();
    for h in handles {
        let loaded = h.await.unwrap();
        assert_eq!(loaded.user_id, "testuser");
    }
}
