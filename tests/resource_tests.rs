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
async fn test_resource_list_with_filters() {
    let server = MockServer::start().await;
    mock_login_and_app(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/resource/ToDo"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data": []}"#))
        .mount(&server)
        .await;

    let mut client = setup_client(&server).await;
    client.authenticate().await.unwrap();

    let builder = client
        .doctype("ToDo")
        .filter("status", "=", "Open")
        .limit(5);
    let raw_result = builder.execute_raw().await;
    println!("Raw result: {:?}", raw_result);

    let result = builder.execute::<serde_json::Value>().await.unwrap();
    assert_eq!(result["data"].as_array().unwrap().len(), 0);
}
#[tokio::test]
async fn test_get_doc() {
    let server = MockServer::start().await;
    mock_login_and_app(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/resource/ToDo/testdoc"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data": {"name": "testdoc"}}"#),
        )
        .mount(&server)
        .await;

    let mut client = setup_client(&server).await;
    client.authenticate().await.unwrap();

    let doc = client.get_doc("ToDo", "testdoc").await.unwrap();
    assert!(doc.contains("testdoc"));
}
