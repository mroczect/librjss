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
        timeout_secs: 5,
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
            r#"
            <html>
            <script>frappe.csrf_token = "mock_csrf_token";</script>
            <script>
            frappe.boot = {
                "user": {
                    "name": "test@example.com",
                    "full_name": "Test User",
                    "roles": ["System Manager"]
                },
                "sitename": "testsite"
            };
            </script>
            </html>
            "#,
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_upload_file() {
    let server = MockServer::start().await;
    mock_login_and_app(&server).await;

    Mock::given(method("POST"))
        .and(path("/api/method/upload_file"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"file_url": "/files/test.png"}"#),
        )
        .mount(&server)
        .await;

    let mut client = setup_client(&server).await;
    client.authenticate().await.unwrap();
    let test_resp = client.authenticated_get("/api/method/upload_file").await;
    println!("GET test: {:?}", test_resp);

    let upload_url = format!("{}/api/method/upload_file", client.base_url());
    println!("Uploading to: {}", upload_url);

    let result = client
        .upload_file("test.png", vec![1, 2, 3], "ToDo", "doc123", "image")
        .await
        .unwrap();
    assert!(result.contains("file_url"));
}
