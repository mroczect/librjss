use librjss::handler::config::{AuthConfig, SameSite};
use librjss::handler::error::AuthError;
use std::env;

fn clear_vars() {
    let vars = [
        "AUTH_COOKIE_NAME",
        "AUTH_COOKIE_PATH",
        "AUTH_COOKIE_DOMAIN",
        "AUTH_COOKIE_SECURE",
        "AUTH_COOKIE_HTTP_ONLY",
        "AUTH_COOKIE_SAME_SITE",
        "AUTH_SESSION_LIFETIME",
        "AUTH_SESSION_IDLE_TIMEOUT",
        "AUTH_LOGIN_URL",
        "AUTH_LOGOUT_REDIRECT_URL",
    ];
    for v in &vars {
        unsafe { env::remove_var(v) };
    }
}

fn with_var<R>(key: &str, val: &str, f: impl FnOnce() -> R) -> R {
    unsafe { env::set_var(key, val) };
    let r = f();
    unsafe { env::remove_var(key) };
    r
}

#[test]
fn test_env_all_vars_set() {
    clear_vars();
    with_var("AUTH_COOKIE_NAME", "mycookie", || {
        with_var("AUTH_COOKIE_PATH", "/secret", || {
            with_var("AUTH_COOKIE_DOMAIN", "dev.local", || {
                with_var("AUTH_COOKIE_SECURE", "false", || {
                    with_var("AUTH_COOKIE_HTTP_ONLY", "false", || {
                        with_var("AUTH_COOKIE_SAME_SITE", "strict", || {
                            with_var("AUTH_SESSION_LIFETIME", "3600", || {
                                with_var("AUTH_SESSION_IDLE_TIMEOUT", "900", || {
                                    with_var("AUTH_LOGIN_URL", "/my-login", || {
                                        with_var("AUTH_LOGOUT_REDIRECT_URL", "/my-logout", || {
                                            let cfg =
                                                AuthConfig::from_env().expect("should succeed");
                                            assert_eq!(cfg.cookie_name, "mycookie");
                                            assert_eq!(cfg.cookie_path, "/secret");
                                            assert_eq!(
                                                cfg.cookie_domain.as_deref(),
                                                Some("dev.local")
                                            );
                                            assert!(!cfg.cookie_secure);
                                            assert!(!cfg.cookie_http_only);
                                            assert_eq!(cfg.cookie_same_site, SameSite::Strict);
                                            assert_eq!(
                                                cfg.session_lifetime,
                                                time::Duration::seconds(3600)
                                            );
                                            assert_eq!(
                                                cfg.session_idle_timeout,
                                                Some(time::Duration::seconds(900))
                                            );
                                            assert_eq!(cfg.login_url.as_deref(), Some("/my-login"));
                                            assert_eq!(
                                                cfg.logout_redirect_url.as_deref(),
                                                Some("/my-logout")
                                            );
                                        })
                                    })
                                })
                            })
                        })
                    })
                })
            })
        })
    });
}

#[test]
fn test_env_missing_optional_vars() {
    clear_vars();
    with_var("AUTH_COOKIE_NAME", "test", || {
        let cfg = AuthConfig::from_env().unwrap();
        assert_eq!(cfg.cookie_name, "test");
        assert_eq!(cfg.cookie_path, "/");
        assert!(cfg.cookie_secure);
        assert_eq!(cfg.session_lifetime, time::Duration::hours(24));
        assert!(cfg.session_idle_timeout.is_none());
    });
}

#[test]
fn test_env_invalid_bool_secure() {
    clear_vars();
    let err = with_var("AUTH_COOKIE_SECURE", "yes", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_COOKIE_SECURE")));
}

#[test]
fn test_env_invalid_bool_http_only() {
    clear_vars();
    let err = with_var("AUTH_COOKIE_HTTP_ONLY", "0", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_COOKIE_HTTP_ONLY")));
}

#[test]
fn test_env_invalid_same_site() {
    clear_vars();
    let err = with_var("AUTH_COOKIE_SAME_SITE", "Strict-ish", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(_)));
}

#[test]
fn test_env_invalid_session_lifetime_not_integer() {
    clear_vars();
    let err = with_var("AUTH_SESSION_LIFETIME", "abc", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_SESSION_LIFETIME")));
}

#[test]
fn test_env_session_lifetime_zero() {
    clear_vars();
    let err = with_var("AUTH_SESSION_LIFETIME", "0", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(ref m) if m.contains("positive")));
}

#[test]
fn test_env_session_idle_timeout_negative() {
    clear_vars();
    let err = with_var("AUTH_SESSION_IDLE_TIMEOUT", "-5", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(_)));
}

#[test]
fn test_env_session_idle_timeout_zero() {
    clear_vars();
    let err = with_var("AUTH_SESSION_IDLE_TIMEOUT", "0", || {
        AuthConfig::from_env().unwrap_err()
    });
    assert!(matches!(err, AuthError::Config(ref m) if m.contains("positive")));
}

#[test]
fn test_env_samesite_case_insensitive() {
    clear_vars();
    let cfg = with_var("AUTH_COOKIE_SAME_SITE", "LAX", || {
        AuthConfig::from_env().unwrap()
    });
    assert_eq!(cfg.cookie_same_site, SameSite::Lax);

    let cfg = with_var("AUTH_COOKIE_SAME_SITE", "None", || {
        AuthConfig::from_env().unwrap()
    });
    assert_eq!(cfg.cookie_same_site, SameSite::None);
}
