use librjss::handler::config::{AuthConfig, SameSite};
use librjss::handler::error::AuthError;
use std::env;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn with_var<R>(key: &str, val: &str, test: impl FnOnce() -> R) -> R {
    let _guard = ENV_MUTEX.lock().unwrap();
    let old = env::var(key).ok();
    unsafe { env::set_var(key, val) };
    let result = test();
    match old {
        Some(v) => unsafe { env::set_var(key, &v) },
        None => unsafe { env::remove_var(key) },
    }
    result
}

#[test]
fn test_env_all_vars_set() {
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
    {
        let _guard = ENV_MUTEX.lock().unwrap();
        let old: Vec<(&str, Option<String>)> =
            vars.iter().map(|&k| (k, env::var(k).ok())).collect();
        unsafe {
            env::set_var("AUTH_COOKIE_NAME", "mycookie");
        }
        unsafe {
            env::set_var("AUTH_COOKIE_PATH", "/secret");
        }
        unsafe {
            env::set_var("AUTH_COOKIE_DOMAIN", "dev.local");
        }
        unsafe {
            env::set_var("AUTH_COOKIE_SECURE", "false");
        }
        unsafe {
            env::set_var("AUTH_COOKIE_HTTP_ONLY", "false");
        }
        unsafe {
            env::set_var("AUTH_COOKIE_SAME_SITE", "strict");
        }
        unsafe {
            env::set_var("AUTH_SESSION_LIFETIME", "3600");
        }
        unsafe {
            env::set_var("AUTH_SESSION_IDLE_TIMEOUT", "900");
        }
        unsafe {
            env::set_var("AUTH_LOGIN_URL", "/my-login");
        }
        unsafe {
            env::set_var("AUTH_LOGOUT_REDIRECT_URL", "/my-logout");
        }

        let cfg = AuthConfig::from_env().expect("should succeed");
        assert_eq!(cfg.cookie_name, "mycookie");
        assert_eq!(cfg.cookie_path, "/secret");
        assert_eq!(cfg.cookie_domain.as_deref(), Some("dev.local"));
        assert!(!cfg.cookie_secure);
        assert!(!cfg.cookie_http_only);
        assert_eq!(cfg.cookie_same_site, SameSite::Strict);
        assert_eq!(cfg.session_lifetime, time::Duration::seconds(3600));
        assert_eq!(cfg.session_idle_timeout, Some(time::Duration::seconds(900)));
        assert_eq!(cfg.login_url.as_deref(), Some("/my-login"));
        assert_eq!(cfg.logout_redirect_url.as_deref(), Some("/my-logout"));

        for (key, old_val) in old {
            match old_val {
                Some(v) => unsafe { env::set_var(key, &v) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}

#[test]
fn test_env_missing_optional_vars() {
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
    with_var("AUTH_COOKIE_SECURE", "yes", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_COOKIE_SECURE")));
    });
}

#[test]
fn test_env_invalid_bool_http_only() {
    with_var("AUTH_COOKIE_HTTP_ONLY", "0", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_COOKIE_HTTP_ONLY")));
    });
}

#[test]
fn test_env_invalid_same_site() {
    with_var("AUTH_COOKIE_SAME_SITE", "Strict-ish", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(_)));
    });
}

#[test]
fn test_env_invalid_session_lifetime_not_integer() {
    with_var("AUTH_SESSION_LIFETIME", "abc", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(ref m) if m.contains("AUTH_SESSION_LIFETIME")));
    });
}

#[test]
fn test_env_session_lifetime_zero() {
    with_var("AUTH_SESSION_LIFETIME", "0", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(ref m) if m.contains("positive")));
    });
}

#[test]
fn test_env_session_idle_timeout_negative() {
    with_var("AUTH_SESSION_IDLE_TIMEOUT", "-5", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(_)));
    });
}

#[test]
fn test_env_session_idle_timeout_zero() {
    with_var("AUTH_SESSION_IDLE_TIMEOUT", "0", || {
        let err = AuthConfig::from_env().unwrap_err();
        assert!(matches!(err, AuthError::Config(ref m) if m.contains("positive")));
    });
}

#[test]
fn test_env_samesite_case_insensitive() {
    with_var("AUTH_COOKIE_SAME_SITE", "LAX", || {
        let cfg = AuthConfig::from_env().unwrap();
        assert_eq!(cfg.cookie_same_site, SameSite::Lax);
    });
    with_var("AUTH_COOKIE_SAME_SITE", "None", || {
        let cfg = AuthConfig::from_env().unwrap();
        assert_eq!(cfg.cookie_same_site, SameSite::None);
    });
}
