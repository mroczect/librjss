use librjss::handler::types::{Credentials, HttpResponse, SessionId, SessionInfo, UserInfo};
use time::{Duration, OffsetDateTime};

#[test]
fn test_session_id_display_and_as_str() {
    let id = SessionId::new("id123".into());
    assert_eq!(id.as_str(), "id123");
    assert_eq!(id.to_string(), "id123");
}

#[test]
fn test_session_id_equality() {
    let a = SessionId::new("same".into());
    let b = SessionId::new("same".into());
    assert_eq!(a, b);
}

#[test]
fn test_session_id_clone() {
    let a = SessionId::new("clone".into());
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_session_id_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let a = SessionId::new("test".into());
    let b = SessionId::new("test".into());
    let mut ha = DefaultHasher::new();
    a.hash(&mut ha);
    let mut hb = DefaultHasher::new();
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn test_http_response_new() {
    let resp = HttpResponse::new(http::StatusCode::OK, "body".into());
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "body");
    assert!(resp.headers.is_empty());
}

#[test]
fn test_http_response_json() {
    let resp = HttpResponse::json(
        http::StatusCode::CREATED,
        serde_json::json!({"key": "value"}),
    );
    assert_eq!(resp.status, 201);
    assert!(resp.body.contains("\"key\":\"value\""));
    assert!(
        resp.headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "application/json")
    );
}

#[test]
fn test_http_response_with_header() {
    let resp = HttpResponse::new(http::StatusCode::OK, "".into())
        .with_header("X-Foo".into(), "bar".into());
    assert_eq!(resp.headers.len(), 1);
    assert_eq!(resp.headers[0].0, "X-Foo");
    assert_eq!(resp.headers[0].1, "bar");
}

#[test]
fn test_http_response_multiple_headers_order() {
    let resp = HttpResponse::new(http::StatusCode::OK, "".into())
        .with_header("A".into(), "1".into())
        .with_header("B".into(), "2".into());
    assert_eq!(resp.headers.len(), 2);
    assert_eq!(resp.headers[0].0, "A");
    assert_eq!(resp.headers[1].0, "B");
}

#[test]
fn test_serialization_credentials() {
    let creds = Credentials {
        username: "alice".into(),
        password: "s3cret".into(),
    };
    let json = serde_json::to_string(&creds).unwrap();
    let back: Credentials = serde_json::from_str(&json).unwrap();
    assert_eq!(back.username, "alice");
    assert_eq!(back.password, "s3cret");
}

#[test]
fn test_serialization_user_info() {
    let info = UserInfo {
        user_id: "42".into(),
        extra: serde_json::json!({"role":"admin"}),
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: UserInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.user_id, "42");
    assert_eq!(back.extra["role"], "admin");
}

#[test]
fn test_serialization_session_info() {
    let now = OffsetDateTime::now_utc();
    let info = SessionInfo {
        user_id: "user".into(),
        data: serde_json::json!({"a":1}),
        issued_at: now,
        expires_at: now + Duration::hours(1),
        idle_deadline: Some(now + Duration::minutes(30)),
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: SessionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.user_id, "user");
    assert_eq!(back.data, serde_json::json!({"a":1}));
    assert!((back.issued_at - now).whole_seconds().abs() <= 1);
    assert!(back.expires_at > now);
    assert!(back.idle_deadline.unwrap() > now);
}

#[test]
fn test_session_info_serde_without_idle_deadline() {
    let info = SessionInfo {
        user_id: "u".into(),
        data: serde_json::Value::Null,
        issued_at: OffsetDateTime::now_utc(),
        expires_at: OffsetDateTime::now_utc() + Duration::hours(1),
        idle_deadline: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: SessionInfo = serde_json::from_str(&json).unwrap();
    assert!(back.idle_deadline.is_none());
}
