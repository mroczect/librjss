use librjss::handler::config::{AuthConfig, SameSite};
use librjss::handler::error::AuthError;
use time::Duration;

#[test]
fn test_default_config() {
    let cfg = AuthConfig::default();
    assert_eq!(cfg.cookie_name, "sid");
    assert_eq!(cfg.cookie_path, "/");
    assert!(cfg.cookie_domain.is_none());
    assert!(cfg.cookie_secure);
    assert!(cfg.cookie_http_only);
    assert_eq!(cfg.cookie_same_site, SameSite::Lax);
    assert_eq!(cfg.session_lifetime, Duration::hours(24));
    assert!(cfg.session_idle_timeout.is_none());
    assert!(cfg.login_url.is_none());
    assert!(cfg.logout_redirect_url.is_none());
}

#[test]
fn test_builder_all_fields() {
    let cfg = AuthConfig::builder()
        .cookie_name("my_sid".into())
        .cookie_path("/app".into())
        .cookie_domain("example.com".into())
        .cookie_secure(false)
        .cookie_http_only(false)
        .cookie_same_site(SameSite::Strict)
        .session_lifetime(Duration::minutes(30))
        .session_idle_timeout(Duration::minutes(5))
        .login_url("/login".into())
        .logout_redirect_url("/bye".into())
        .build()
        .expect("valid config");

    assert_eq!(cfg.cookie_name, "my_sid");
    assert_eq!(cfg.cookie_path, "/app");
    assert_eq!(cfg.cookie_domain.as_deref(), Some("example.com"));
    assert!(!cfg.cookie_secure);
    assert!(!cfg.cookie_http_only);
    assert_eq!(cfg.cookie_same_site, SameSite::Strict);
    assert_eq!(cfg.session_lifetime, Duration::minutes(30));
    assert_eq!(cfg.session_idle_timeout, Some(Duration::minutes(5)));
    assert_eq!(cfg.login_url.as_deref(), Some("/login"));
    assert_eq!(cfg.logout_redirect_url.as_deref(), Some("/bye"));
}

#[test]
fn test_builder_validation_lifetime_zero() {
    let err = AuthConfig::builder()
        .session_lifetime(Duration::seconds(0))
        .build()
        .unwrap_err();
    assert!(matches!(err, AuthError::Config(ref msg) if msg.contains("positive")));
}

#[test]
fn test_builder_validation_cookie_name_empty() {
    let err = AuthConfig::builder()
        .cookie_name("".into())
        .build()
        .unwrap_err();
    assert!(matches!(err, AuthError::Config(ref msg) if msg.contains("cookie_name")));
}

#[test]
fn test_builder_partial_overrides() {
    let cfg = AuthConfig::builder()
        .cookie_name("custom".into())
        .session_lifetime(Duration::minutes(10))
        .build()
        .unwrap();
    assert_eq!(cfg.cookie_name, "custom");
    assert_eq!(cfg.cookie_path, "/");
    assert_eq!(cfg.session_lifetime, Duration::minutes(10));
}

#[test]
fn test_builder_samesite_serialization() {
    let lax: SameSite = serde_json::from_str("\"Lax\"").unwrap();
    assert_eq!(lax, SameSite::Lax);
    let json = serde_json::to_string(&SameSite::Strict).unwrap();
    assert_eq!(json, "\"Strict\"");
}

#[test]
fn test_builder_edge_case_lifetime_one_second() {
    let cfg = AuthConfig::builder()
        .session_lifetime(Duration::seconds(1))
        .build()
        .unwrap();
    assert_eq!(cfg.session_lifetime, Duration::seconds(1));
}
