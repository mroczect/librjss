use librjss::RjssClient;
use librjss::handler::config::{AuthMode, ClientConfig};
use librjss::handler::error::JuraganError;
use librjss::handler::types::SessionInfo;
use reqwest::Url;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_app_html(sitename: &str, full_name: &str, roles: Vec<&str>) -> String {
    let roles_json: Vec<String> = roles.iter().map(|r| format!("\"{}\"", r)).collect();
    let roles_str = roles_json.join(",");
    format!(
        r#"
        <html>
        <script>frappe.csrf_token = "mock_csrf_token";</script>
        <script>
        frappe.boot = {{
            "user": {{
                "name": "test@example.com",
                "full_name": "{full_name}",
                "roles": [{roles}]
            }},
            "sitename": "{sitename}"
        }};
        </script>
        </html>
        "#,
        full_name = full_name,
        roles = roles_str,
        sitename = sitename
    )
}

fn session_auth(email: &str, password: &str) -> AuthMode {
    AuthMode::Session {
        email: SecretString::new(Box::from(email.to_string())),
        password: SecretString::new(Box::from(password.to_string())),
    }
}

fn token_auth(api_key: &str, api_secret: &str) -> AuthMode {
    AuthMode::Token {
        api_key: api_key.to_string(),
        api_secret: SecretString::new(Box::from(api_secret.to_string())),
    }
}

async fn setup_client(server: &MockServer, auth_mode: AuthMode) -> RjssClient {
    let config = ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        auth_mode,
        expected_sitename: None,
        required_roles: vec![],
        timeout_secs: 5,
        max_retries: 1,
        user_agent: "test".into(),
        insecure_ssl: true,
    };
    RjssClient::new(config).unwrap()
}

#[tokio::test]
async fn test_authenticate_session_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"message\":\"Logged In\",\"full_name\":\"Test User\"}"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/app"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_app_html(
            "testsite",
            "Test User",
            vec!["System Manager"],
        )))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    client.authenticate().await.unwrap();

    let info = client.session_info().unwrap();
    assert_eq!(info.full_name, Some("Test User".into()));
    assert_eq!(info.sitename, "testsite");
    assert_eq!(info.roles, vec!["System Manager"]);
    assert_eq!(info.csrf_token.expose_secret(), "mock_csrf_token");
}

#[tokio::test]
async fn test_authenticate_token_mode() {
    let server = MockServer::start().await;
    let mut client = setup_client(&server, token_auth("key", "secret")).await;

    client.authenticate().await.unwrap();
    let info = client.session_info().unwrap();
    assert_eq!(info.sid.expose_secret(), "token-mode");
}

#[tokio::test]
async fn test_authenticate_rate_limited() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    let result = client.authenticate().await;
    assert!(matches!(result, Err(JuraganError::RateLimited)));
}

#[tokio::test]
async fn test_authenticate_login_http_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    let result = client.authenticate().await;
    assert!(matches!(result, Err(JuraganError::Auth(_))));
}

#[tokio::test]
async fn test_authenticated_get_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"message\":\"Logged In\"}"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/app"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_app_html(
            "site",
            "User",
            vec![],
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/resource/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string("resource data"))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    client.authenticate().await.unwrap();

    let body = client
        .authenticated_get("/api/resource/test")
        .await
        .unwrap();
    assert_eq!(body, "resource data");
}

#[tokio::test]
async fn test_authenticated_post_with_csrf() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"message\":\"Logged In\"}"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/app"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mock_app_html(
            "site",
            "User",
            vec![],
        )))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/resource/test"))
        .and(wiremock::matchers::header(
            "X-Frappe-CSRF-Token",
            "mock_csrf_token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string("created"))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    client.authenticate().await.unwrap();

    let body = client
        .authenticated_post("/api/resource/test", "{}")
        .await
        .unwrap();
    assert_eq!(body, "created");
}

#[tokio::test]
async fn test_authenticated_request_path_traversal() {
    let server = MockServer::start().await;
    let client = setup_client(&server, token_auth("k", "s")).await;
    let result = client.authenticated_get("/../etc/passwd").await;
    assert!(matches!(result, Err(JuraganError::Validation(_))));
}

#[tokio::test]
async fn test_logout_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/method/logout"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    client.session = Some(SessionInfo {
        sid: SecretString::new(Box::from("sid".into())),
        csrf_token: SecretString::new(Box::from("csrf".into())),
        full_name: None,
        sitename: "".into(),
        roles: vec![],
    });

    client.logout().await.unwrap();
    assert!(client.session_info().is_none());
}

#[tokio::test]
async fn test_logout_not_authenticated() {
    let server = MockServer::start().await;
    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    let result = client.logout().await;
    assert!(matches!(result, Err(JuraganError::NotAuthenticated)));
}

#[tokio::test]
async fn test_ensure_session_valid() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/method/frappe.auth.get_logged_user"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let mut client = setup_client(&server, session_auth("user", "pass")).await;
    client.session = Some(SessionInfo {
        sid: SecretString::new(Box::from("sid".into())),
        csrf_token: SecretString::new(Box::from("csrf".into())),
        full_name: None,
        sitename: "".into(),
        roles: vec![],
    });

    client.ensure_session().await.unwrap();
}
