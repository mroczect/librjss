use librjss::api::auth::AuthEndpoints;
use reqwest::Url;

struct TestEndpoints;
impl AuthEndpoints for TestEndpoints {}

#[test]
fn test_login_url() {
    let base = Url::parse("https://example.com").unwrap();
    let url = TestEndpoints::login_url(&base);
    assert_eq!(url.as_str(), "https://example.com/api/method/login");

    let base_trailing = Url::parse("https://example.com/").unwrap();
    let url2 = TestEndpoints::login_url(&base_trailing);
    assert_eq!(url2.as_str(), "https://example.com/api/method/login");
}

#[test]
fn test_logout_url() {
    let base = Url::parse("https://example.com").unwrap();
    let url = TestEndpoints::logout_url(&base);
    assert_eq!(url.as_str(), "https://example.com/api/method/logout");
}

#[test]
fn test_csrf_token_url() {
    let base = Url::parse("https://example.com").unwrap();
    let url = TestEndpoints::csrf_token_url(&base);
    assert_eq!(
        url.as_str(),
        "https://example.com/api/method/frappe.auth.get_csrf_token"
    );
}

#[test]
fn test_get_logged_user_url() {
    let base = Url::parse("https://example.com").unwrap();
    let url = TestEndpoints::get_logged_user_url(&base);
    assert_eq!(
        url.as_str(),
        "https://example.com/api/method/frappe.auth.get_logged_user"
    );
}

#[test]
fn test_app_page_url() {
    let base = Url::parse("https://example.com").unwrap();
    let url = TestEndpoints::app_page_url(&base);
    assert_eq!(url.as_str(), "https://example.com/app");
}
