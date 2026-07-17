use librjss::handler::env::{AuthMode, ClientConfig};
use secrecy::SecretString;
use url::Url;

fn make_config(scheme: &str, insecure: bool, email: &str, password: &str) -> ClientConfig {
    ClientConfig {
        base_url: Url::parse(&format!("{}://example.com", scheme)).unwrap(),
        auth_mode: AuthMode::Session {
            email: SecretString::new(Box::from(email.to_string())),
            password: SecretString::new(Box::from(password.to_string())),
        },
        expected_sitename: None,
        required_roles: vec![],
        timeout_secs: 30,
        max_retries: 3,
        user_agent: "test".into(),
        insecure_ssl: insecure,
    }
}

#[test]
fn test_https_required() {
    let cfg = make_config("http", false, "a@b.com", "pass");
    assert!(cfg.validate().is_err());
}

#[test]
fn test_http_allowed_when_insecure() {
    let cfg = make_config("http", true, "a@b.com", "pass");
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_https_always_ok() {
    let cfg = make_config("https", false, "a@b.com", "pass");
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_empty_email() {
    let cfg = make_config("https", false, "", "pass");
    assert!(cfg.validate().is_err());
}

#[test]
fn test_empty_password() {
    let cfg = make_config("https", false, "a@b.com", "");
    assert!(cfg.validate().is_err());
}

#[test]
fn test_token_mode_no_validation() {
    let cfg = ClientConfig {
        base_url: Url::parse("http://example.com").unwrap(),
        auth_mode: AuthMode::Token {
            api_key: "key".into(),
            api_secret: SecretString::new(Box::from("secret".to_string())),
        },
        expected_sitename: None,
        required_roles: vec![],
        timeout_secs: 10,
        max_retries: 1,
        user_agent: "test".into(),
        insecure_ssl: true,
    };
    assert!(cfg.validate().is_ok());
}
