use librjss::RjssClient;
use librjss::handler::config::{AuthMode, ClientConfig};
use reqwest::Url;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn session_auth(email: &str, password: &str) -> AuthMode {
    AuthMode::Session {
        email: SecretString::new(Box::from(email.to_string())),
        password: SecretString::new(Box::from(password.to_string())),
    }
}

async fn setup_client(server: &MockServer) -> RjssClient {
    let config = ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        auth_mode: session_auth("user", "pass"),
        expected_sitename: None,
        required_roles: vec![],
        timeout_secs: 1, // <-- lebih pendek supaya cepat gagal kalau salah
        max_retries: 1,
        user_agent: "test".into(),
        insecure_ssl: true,
    };
    RjssClient::new(config).unwrap()
}

async fn mock_login_and_app(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/method/login"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"message\":\"Logged In\",\"full_name\":\"Test User\"}"),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/app"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<html>
            <script>frappe.csrf_token = "mock_csrf_token";</script>
            <script>
            frappe.boot = {
                "user": { "name": "t","full_name":"T","roles":["R"],"can_read":["ToDo"] },
                "sitename": "t","csrf_token": "mock_csrf_token",
                "user_info": {},
                "sidebar_pages": {"pages":[],"has_access":false,"has_create_access":false},
                "navbar_settings": null,
                "versions": {"frappe":"16"},
                "lang_dict": {},"lang":"en",
                "page_info": {},
                "frequently_visited_links": [],
                "developer_mode": 0, "read_only": false, "desk_theme": "Light"
            };
            </script></html>"#,
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_upload_file() {
    let server = MockServer::start().await;
    mock_login_and_app(&server).await;

    // Mock untuk endpoint upload – cukup cocokkan method POST dan path
    Mock::given(method("POST"))
        .and(path("/api/method/upload_file"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"file_url":"/files/test.png"}"#),
        )
        .mount(&server)
        .await;

    let mut client = setup_client(&server).await;
    client.authenticate().await.unwrap();

    // Hapus panggilan authenticated_get yang tidak perlu – itu yang bikin lambat
    let result = client
        .upload_file("test.png", vec![1, 2, 3], "ToDo", "doc123", "image")
        .await
        .unwrap();
    assert!(result.contains("file_url"));
}
