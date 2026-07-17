use librjss::RjssClient;
use librjss::handler::config::{AuthMode, ClientConfig};
use reqwest::Url;
use secrecy::SecretString;
use wiremock::matchers::{body_string_contains, method, path};
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
            r#"<html>
            <script>frappe.csrf_token = "mock_csrf_token";</script>
            <script>
            frappe.boot = {
                "user": {
                    "name": "test@example.com",
                    "full_name": "Test User",
                    "roles": ["System Manager"],
                    "can_read": ["ToDo"]
                },
                "sitename": "testsite",
                "csrf_token": "mock_csrf_token",
                "user_info": {},
                "sidebar_pages": {"pages": [], "has_access": false, "has_create_access": false},
                "navbar_settings": null,
                "versions": {"frappe": "16.0.0"},
                "lang_dict": {},
                "lang": "en",
                "page_info": {},
                "frequently_visited_links": [],
                "developer_mode": 0,
                "read_only": false,
                "desk_theme": "Light"
            };
            </script>
            </html>"#,
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_run_report() {
    let server = MockServer::start().await;
    mock_login_and_app(&server).await;

    Mock::given(method("POST"))
        .and(path("/api/method/frappe.desk.query_report.run"))
        .and(body_string_contains("\"report_name\":\"Test Report\""))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"result": []}"#))
        .mount(&server)
        .await;

    let mut client = setup_client(&server).await;
    client.authenticate().await.unwrap();

    let result = client
        .report("Test Report")
        .add_filter("year", "2026")
        .run::<serde_json::Value>()
        .await
        .unwrap();
    assert!(result["result"].as_array().is_some());
}
