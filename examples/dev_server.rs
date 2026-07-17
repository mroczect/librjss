use axum::{
    Router,
    extract::Form,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize)]
struct LoginForm {
    usr: String,
    pwd: String,
}

#[derive(Serialize)]
struct LoginResponse {
    message: String,
    home_page: String,
    full_name: String,
}

#[derive(Serialize)]
struct UserInfoResponse {
    message: UserInfoMessage,
}

#[derive(Serialize)]
struct UserInfoMessage {
    name: String,
    email: Option<String>,
    roles: Vec<String>,
}

#[derive(Serialize)]
struct CsrfErrorResponse {
    exception: String,
    exc_type: String,
    #[serde(rename = "_server_messages")]
    server_messages: String,
}

fn login_page_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <title>JSS - Login</title>
    <script>
        window.frappe = {};
        frappe.csrf_token = "None";
    </script>
</head>
<body>
    <div class="login-container">
        <form class="form-login">
            <input type="text" name="usr" placeholder="Email">
            <input type="password" name="pwd" placeholder="Password">
            <button type="submit">Sign In</button>
        </form>
    </div>
</body>
</html>"#
        .to_string()
}

fn app_page_html(sitename: &str, full_name: &str, roles: &[&str], csrf_token: &str) -> String {
    let roles_json: Vec<String> = roles.iter().map(|r| format!("\"{}\"", r)).collect();
    let roles_str = roles_json.join(",");

    format!(
        r#"<!DOCTYPE html>
<html data-theme-mode="light" lang="en">
<head><title>JSS - Dashboard</title></head>
<body>
    <script>frappe.csrf_token = "{csrf_token}";</script>
    <script>
    frappe.boot = {{
        "user": {{
            "name": "dev@example.com",
            "email": "dev@example.com",
            "full_name": "{full_name}",
            "first_name": "{full_name}",
            "roles": [{roles}],
            "allow_modules": [],
            "user_type": "System User",
            "permissions": {{}}
        }},
        "sitename": "{sitename}",
        "csrf_token": "{csrf_token}",
        "sysdefaults": {{
            "default_app": "juragan",
            "time_zone": "Asia/Jakarta"
        }}
    }};
    </script>
</body>
</html>"#,
        full_name = full_name,
        roles = roles_str,
        sitename = sitename,
        csrf_token = csrf_token,
    )
}

async fn login_handler(Form(form): Form<LoginForm>) -> impl IntoResponse {
    if form.usr.is_empty() || form.pwd.is_empty() {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "message": "Invalid credentials"
            })),
        )
            .into_response();
    }

    let response = LoginResponse {
        message: "Logged In".to_string(),
        home_page: "/app".to_string(),
        full_name: "Developer User".to_string(),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        "sid=dev_sid_abc123xyz; Expires=Sat, 18 Jul 2026 12:00:00 GMT; Max-Age=86400; HttpOnly; Path=/; SameSite=Lax"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "full_name=Developer%20User; Path=/; SameSite=Lax"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "system_user=yes; Path=/; SameSite=Lax".parse().unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "user_id=dev%40example.com; Path=/; SameSite=Lax"
            .parse()
            .unwrap(),
    );

    (StatusCode::OK, headers, Json(response)).into_response()
}

async fn app_handler() -> impl IntoResponse {
    let html = app_page_html(
        "dev-site",
        "Developer User",
        &["System Manager", "Employee"],
        "dev_csrf_token_abc123",
    );

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/html; charset=utf-8".parse().unwrap());

    (headers, Html(html))
}

async fn get_logged_user_handler() -> Json<UserInfoResponse> {
    Json(UserInfoResponse {
        message: UserInfoMessage {
            name: "dev@example.com".to_string(),
            email: Some("dev@example.com".to_string()),
            roles: vec!["System Manager".to_string(), "Employee".to_string()],
        },
    })
}

async fn logout_handler() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        "sid=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "full_name=; Path=/; Max-Age=0; SameSite=Lax"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "system_user=; Path=/; Max-Age=0; SameSite=Lax"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Set-Cookie",
        "user_id=; Path=/; Max-Age=0; SameSite=Lax".parse().unwrap(),
    );

    (StatusCode::OK, headers, "Logged Out")
}

async fn csrf_handler() -> impl IntoResponse {
    (
        StatusCode::EXPECTATION_FAILED,
        Json(CsrfErrorResponse {
            exception: "frappe.exceptions.ValidationError: Failed to get method for command frappe.auth.get_csrf_token".to_string(),
            exc_type: "ValidationError".to_string(),
            server_messages: r#"[{"message":"Failed to get method for command frappe.auth.get_csrf_token","title":"Message","indicator":"red","raise_exception":1}]"#.to_string(),
        }),
    )
}

async fn index_handler() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/html; charset=utf-8".parse().unwrap());
    (headers, Html(login_page_html()))
}

async fn fallback_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"message": "Not Found"})),
    )
}
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/login", get(index_handler))
        .route("/api/method/login", post(login_handler))
        .route("/app", get(app_handler))
        .route(
            "/api/method/frappe.auth.get_logged_user",
            get(get_logged_user_handler),
        )
        .route("/api/method/logout", post(logout_handler))
        .route("/api/method/frappe.auth.get_csrf_token", get(csrf_handler))
        .fallback(fallback_handler);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("JSS Mock Server running at http://{addr}");
    println!("Set env: JSS_BASE_URL=http://{addr}");
    println!("Then: cargo run --bin cli_auth");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
