use super::RjssClient;
use crate::api::auth::AuthEndpoints;
use crate::handler::env::AuthMode;
use crate::handler::error::JuraganError;
use crate::handler::types::{LoginApiResponse, SessionInfo};
use backoff::{ExponentialBackoff, future::retry};
use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

impl AuthEndpoints for RjssClient {}

impl RjssClient {
    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn authenticate(&mut self) -> Result<(), JuraganError> {
        match self.config.auth_mode.clone() {
            AuthMode::Session { email, password } => {
                self.credentials = Some((email.clone(), password.clone()));
                self.login_with_credentials(email, password).await
            }
            AuthMode::Token {
                api_key: _,
                api_secret: _,
            } => {
                self.session = Some(SessionInfo {
                    sid: SecretString::new(Box::from("token-mode")),
                    csrf_token: SecretString::new(Box::from("not-used")),
                    full_name: None,
                    sitename: self.config.expected_sitename.clone().unwrap_or_default(),
                    roles: vec![],
                });
                info!(trace_id = self.trace_id, "Using token API key");
                Ok(())
            }
        }
    }

    async fn login_with_credentials(
        &mut self,
        email: SecretString,
        password: SecretString,
    ) -> Result<(), JuraganError> {
        let login_url = Self::login_url(&self.config.base_url);

        let email_hash = format!("{:x}", Sha256::digest(email.expose_secret().as_bytes()));
        info!(trace_id = self.trace_id, email_hash = %email_hash, "Login attempt");

        let params = [
            ("usr", email.expose_secret()),
            ("pwd", password.expose_secret()),
        ];

        let resp = self.http.post(login_url).form(&params).send().await?;

        let status = resp.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            error!(trace_id = self.trace_id, "Rate limited");
            return Err(JuraganError::RateLimited);
        }
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            error!(trace_id = self.trace_id, %status, %body, "Login failed");
            return Err(JuraganError::Auth(format!("Login failed: HTTP {status}")));
        }

        let body_text = resp.text().await?;
        error!(trace_id = self.trace_id, %body_text, "Raw login response body");
        debug!(
            trace_id = self.trace_id,
            body_hash = format!("{:x}", Sha256::digest(body_text.as_bytes())),
            "Login response received"
        );

        let v: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| JuraganError::Parse(format!("Login response not valid JSON: {e}")))?;

        let login_resp = if v["message"].as_str() == Some("Logged In") {
            warn!(
                trace_id = self.trace_id,
                "Login response has string message 'Logged In'"
            );
            LoginApiResponse {
                message: crate::handler::types::LoginApiMessage {
                    sid: String::new(),
                    full_name: v["full_name"].as_str().map(|s| s.to_string()),
                },
            }
        } else {
            serde_json::from_value::<LoginApiResponse>(v)
                .map_err(|e| JuraganError::Parse(format!("Failed to parse login response: {e}")))?
        };

        let app_html = self.fetch_app_page().await?;
        let (csrf_token, boot_data) = self.extract_app_data(&app_html)?;

        let user = &boot_data.user;
        let roles = user.roles.clone();
        let full_name = user.full_name.clone().or(login_resp.message.full_name);
        let sitename = boot_data.sitename.clone();

        if let Some(expected) = &self.config.expected_sitename {
            if expected != &sitename {
                error!(trace_id = self.trace_id, expected = %expected, actual = %sitename, "Sitename mismatch");
                return Err(JuraganError::SitenameMismatch {
                    expected: expected.clone(),
                    actual: sitename,
                });
            }
            info!(trace_id = self.trace_id, sitename = %sitename, "Sitename verified");
        }

        if !self.config.required_roles.is_empty() {
            let has_role = self.config.required_roles.iter().any(|r| roles.contains(r));
            if !has_role {
                error!(trace_id = self.trace_id, ?roles, required = ?self.config.required_roles, "Missing required roles");
                return Err(JuraganError::Permission(format!(
                    "Missing one of required roles: {:?}",
                    self.config.required_roles
                )));
            }
        }

        self.session = Some(SessionInfo {
            sid: SecretString::new(Box::from(login_resp.message.sid)),
            csrf_token,
            full_name,
            sitename,
            roles,
        });

        info!(trace_id = self.trace_id, email_hash = %email_hash, "Login successful");
        Ok(())
    }

    async fn obtain_csrf_token(&self) -> Result<SecretString, JuraganError> {
        let csrf_url = Self::csrf_token_url(&self.config.base_url);
        let resp = self.http.get(csrf_url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;

        error!(trace_id = self.trace_id, %status, %body, "Raw CSRF token response");
        if status.is_success() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(token) = json.get("message").and_then(|v| v.as_str()) {
                    return Ok(SecretString::new(Box::from(token.to_owned())));
                }
            }
            warn!(
                trace_id = self.trace_id,
                "CSRF response did not contain expected 'message' field"
            );
        }
        Err(JuraganError::Csrf("Unable to obtain CSRF token".into()))
    }

    async fn fetch_user_info(&self) -> Result<crate::handler::types::UserInfo, JuraganError> {
        let url = Self::get_logged_user_url(&self.config.base_url);
        let resp = self.http.get(url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;

        error!(trace_id = self.trace_id, %status, %body, "Raw user info response");
        if !status.is_success() {
            return Err(JuraganError::Auth("Failed to verify session".into()));
        }
        serde_json::from_str::<crate::handler::types::UserInfo>(&body)
            .map_err(|e| JuraganError::Parse(format!("Failed to parse user info: {e}")))
    }

    async fn fetch_sitename_from_app(&self) -> Result<String, JuraganError> {
        let app_url = Self::app_page_url(&self.config.base_url);
        let resp = self.http.get(app_url).send().await?;
        let body = resp.text().await?;
        let re = regex::Regex::new(r#""sitename"\s*:\s*"([^"]+)""#)
            .map_err(|_| JuraganError::Parse("Regex compilation failed".into()))?;
        if let Some(caps) = re.captures(&body) {
            Ok(caps[1].to_string())
        } else {
            error!(trace_id = self.trace_id, "Sitename not found in /app");
            Err(JuraganError::Parse("Sitename not found in /app".into()))
        }
    }
    async fn fetch_app_page(&self) -> Result<String, JuraganError> {
        let app_url = Self::app_page_url(&self.config.base_url);
        let resp = self.http.get(app_url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        error!(trace_id = self.trace_id, %status, %body, "Raw /app response");
        if !status.is_success() {
            return Err(JuraganError::Auth("Failed to load /app".into()));
        }
        Ok(body)
    }

    fn extract_app_data(
        &self,
        html: &str,
    ) -> Result<(SecretString, crate::handler::types::FrappeBoot), JuraganError> {
        let document = Html::parse_document(html);

        let selector = Selector::parse("script").unwrap();
        let mut csrf_token = String::new();
        for script in document.select(&selector) {
            let text = script.inner_html();
            if let Some(pos) = text.find("frappe.csrf_token") {
                if let Some(start) = text[pos..].find('"') {
                    let start = pos + start + 1;
                    if let Some(end) = text[start..].find('"') {
                        csrf_token = text[start..start + end].to_string();
                        break;
                    }
                }
            }
        }

        let re = regex::Regex::new(r"frappe\.boot\s*=\s*(\{.*?\});\s*\n")
            .map_err(|_| JuraganError::Parse("Regex compilation error".into()))?;
        let caps = re.captures(html).ok_or(JuraganError::Parse(
            "Could not find frappe.boot object in /app".into(),
        ))?;
        let boot_json = caps.get(1).unwrap().as_str();

        let boot: crate::handler::types::FrappeBoot = serde_json::from_str(boot_json)
            .map_err(|e| JuraganError::Parse(format!("Failed to parse frappe.boot: {e}")))?;

        Ok((SecretString::new(Box::from(csrf_token)), boot))
    }

    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn logout(&mut self) -> Result<(), JuraganError> {
        let session = self
            .session
            .as_ref()
            .ok_or(JuraganError::NotAuthenticated)?;
        let csrf = session.csrf_token.expose_secret();
        let logout_url = Self::logout_url(&self.config.base_url);
        let resp = self
            .http
            .post(logout_url)
            .header("X-Frappe-CSRF-Token", csrf)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            info!(trace_id = self.trace_id, "Logout successful");
            self.session = None;
            self.credentials = None;
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            error!(trace_id = self.trace_id, %status, %body, "Logout failed");
            Err(JuraganError::Http { status, body })
        }
    }

    fn apply_auth_to_builder(
        auth_mode: &AuthMode,
        mut req: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let AuthMode::Token {
            api_key,
            api_secret,
        } = auth_mode
        {
            req = req.header(
                "Authorization",
                format!("token {}:{}", api_key, api_secret.expose_secret()),
            );
        }
        req
    }

    pub(crate) fn build_join_url(&self, path: &str) -> Result<reqwest::Url, JuraganError> {
        if path.contains("..") {
            return Err(JuraganError::Validation(
                "Path traversal not allowed".into(),
            ));
        }
        self.config
            .base_url
            .join(path)
            .map_err(|e| JuraganError::Parse(format!("URL join error: {e}")))
    }

    fn backoff_config(&self) -> ExponentialBackoff {
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(
                self.config.timeout_secs * (self.config.max_retries as u64 + 1),
            )),
            ..Default::default()
        }
    }

    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn authenticated_get(&self, path: &str) -> Result<String, JuraganError> {
        let url = self.build_join_url(path)?;
        let http = self.http.clone();
        let auth_mode = self.config.auth_mode.clone();
        let backoff = self.backoff_config();

        let op = || {
            let http = http.clone();
            let auth_mode = auth_mode.clone();
            let url = url.clone();
            async move {
                let mut req = http.get(url);
                req = Self::apply_auth_to_builder(&auth_mode, req);
                let resp = req
                    .send()
                    .await
                    .map_err(|e| backoff::Error::transient(JuraganError::Network(e)))?;
                let status = resp.status();
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    Err(backoff::Error::transient(JuraganError::Http {
                        status,
                        body: String::new(),
                    }))
                } else if status == StatusCode::UNAUTHORIZED {
                    Err(backoff::Error::permanent(JuraganError::Auth(
                        "Unauthorized".into(),
                    )))
                } else {
                    Ok(resp)
                }
            }
        };

        let resp = retry(backoff, op).await?;
        let status = resp.status();
        let body = resp.text().await?;
        debug!(trace_id = self.trace_id, path = %path, status = %status, body_hash = format!("{:x}", Sha256::digest(body.as_bytes())));
        Ok(body)
    }

    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn authenticated_post(
        &self,
        path: &str,
        body_json: &str,
    ) -> Result<String, JuraganError> {
        let url = self.build_join_url(path)?;
        let http = self.http.clone();
        let auth_mode = self.config.auth_mode.clone();
        let backoff = self.backoff_config();
        let csrf_token = self
            .session
            .as_ref()
            .map(|s| s.csrf_token.expose_secret().to_owned());
        let is_session = matches!(&auth_mode, AuthMode::Session { .. });

        let op = || {
            let http = http.clone();
            let auth_mode = auth_mode.clone();
            let url = url.clone();
            let body = body_json.to_owned();
            let csrf = csrf_token.clone();
            async move {
                let mut req = http
                    .post(url)
                    .body(body)
                    .header("Content-Type", "application/json");
                if is_session {
                    if let Some(token) = &csrf {
                        req = req.header("X-Frappe-CSRF-Token", token);
                    }
                }
                req = Self::apply_auth_to_builder(&auth_mode, req);
                let resp = req
                    .send()
                    .await
                    .map_err(|e| backoff::Error::transient(JuraganError::Network(e)))?;
                let status = resp.status();
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    Err(backoff::Error::transient(JuraganError::Http {
                        status,
                        body: String::new(),
                    }))
                } else if status == StatusCode::UNAUTHORIZED {
                    Err(backoff::Error::permanent(JuraganError::Auth(
                        "Unauthorized".into(),
                    )))
                } else {
                    Ok(resp)
                }
            }
        };

        let resp = retry(backoff, op).await?;
        let status = resp.status();
        let body = resp.text().await?;
        debug!(trace_id = self.trace_id, path = %path, status = %status, body_hash = format!("{:x}", Sha256::digest(body.as_bytes())));
        Ok(body)
    }

    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn authenticated_put(
        &self,
        path: &str,
        body_json: &str,
    ) -> Result<String, JuraganError> {
        let url = self.build_join_url(path)?;
        let http = self.http.clone();
        let auth_mode = self.config.auth_mode.clone();
        let backoff = self.backoff_config();
        let csrf_token = self
            .session
            .as_ref()
            .map(|s| s.csrf_token.expose_secret().to_owned());
        let is_session = matches!(&auth_mode, AuthMode::Session { .. });

        let op = || {
            let http = http.clone();
            let auth_mode = auth_mode.clone();
            let url = url.clone();
            let body = body_json.to_owned();
            let csrf = csrf_token.clone();
            async move {
                let mut req = http
                    .put(url)
                    .body(body)
                    .header("Content-Type", "application/json");
                if is_session {
                    if let Some(token) = &csrf {
                        req = req.header("X-Frappe-CSRF-Token", token);
                    }
                }
                req = Self::apply_auth_to_builder(&auth_mode, req);
                let resp = req
                    .send()
                    .await
                    .map_err(|e| backoff::Error::transient(JuraganError::Network(e)))?;
                let status = resp.status();
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    Err(backoff::Error::transient(JuraganError::Http {
                        status,
                        body: String::new(),
                    }))
                } else if status == StatusCode::UNAUTHORIZED {
                    Err(backoff::Error::permanent(JuraganError::Auth(
                        "Unauthorized".into(),
                    )))
                } else {
                    Ok(resp)
                }
            }
        };

        let resp = retry(backoff, op).await?;
        let status = resp.status();
        let body = resp.text().await?;
        debug!(trace_id = self.trace_id, path = %path, status = %status, body_hash = format!("{:x}", Sha256::digest(body.as_bytes())));
        Ok(body)
    }

    #[instrument(skip(self), fields(trace_id = self.trace_id))]
    pub async fn authenticated_delete(&self, path: &str) -> Result<String, JuraganError> {
        let url = self.build_join_url(path)?;
        let http = self.http.clone();
        let auth_mode = self.config.auth_mode.clone();
        let backoff = self.backoff_config();
        let csrf_token = self
            .session
            .as_ref()
            .map(|s| s.csrf_token.expose_secret().to_owned());
        let is_session = matches!(&auth_mode, AuthMode::Session { .. });

        let op = || {
            let http = http.clone();
            let auth_mode = auth_mode.clone();
            let url = url.clone();
            let csrf = csrf_token.clone();
            async move {
                let mut req = http.delete(url);
                if is_session {
                    if let Some(token) = &csrf {
                        req = req.header("X-Frappe-CSRF-Token", token);
                    }
                }
                req = Self::apply_auth_to_builder(&auth_mode, req);
                let resp = req
                    .send()
                    .await
                    .map_err(|e| backoff::Error::transient(JuraganError::Network(e)))?;
                let status = resp.status();
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    Err(backoff::Error::transient(JuraganError::Http {
                        status,
                        body: String::new(),
                    }))
                } else if status == StatusCode::UNAUTHORIZED {
                    Err(backoff::Error::permanent(JuraganError::Auth(
                        "Unauthorized".into(),
                    )))
                } else {
                    Ok(resp)
                }
            }
        };

        let resp = retry(backoff, op).await?;
        let status = resp.status();
        let body = resp.text().await?;
        debug!(trace_id = self.trace_id, path = %path, status = %status, body_hash = format!("{:x}", Sha256::digest(body.as_bytes())));
        Ok(body)
    }

    pub async fn ensure_session(&mut self) -> Result<(), JuraganError> {
        loop {
            if self.session.is_none() {
                if let Some((email, password)) = self.credentials.clone() {
                    self.login_with_credentials(email, password).await?;
                    return Ok(());
                } else {
                    return Err(JuraganError::NotAuthenticated);
                }
            } else {
                let url = Self::get_logged_user_url(&self.config.base_url);
                let resp = self.http.get(url).send().await?;
                if resp.status() == StatusCode::UNAUTHORIZED {
                    warn!(trace_id = self.trace_id, "Session expired, will re-login");
                    self.session = None;
                } else {
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::env::ClientConfig;
    use crate::handler::types::SessionInfo;
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

    async fn setup_test_client(server: &MockServer, auth_mode: AuthMode) -> RjssClient {
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

    async fn mock_authenticated_requests(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid", "full_name": "User"}
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"message": "csrf"})),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "roles": []}
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn test_authenticate_token_mode() {
        let server = MockServer::start().await;
        let config = ClientConfig {
            base_url: Url::parse(&server.uri()).unwrap(),
            auth_mode: AuthMode::Token {
                api_key: "key".into(),
                api_secret: SecretString::new(Box::from("secret".to_string())),
            },
            expected_sitename: Some("expected_site".to_string()),
            required_roles: vec![],
            timeout_secs: 5,
            max_retries: 1,
            user_agent: "test".into(),
            insecure_ssl: true,
        };
        let mut client = RjssClient::new(config).unwrap();
        client.authenticate().await.unwrap();
        assert!(client.session.is_some());
        let session = client.session.as_ref().unwrap();
        assert_eq!(session.sid.expose_secret(), "token-mode");
        assert_eq!(session.sitename, "expected_site");
    }

    #[tokio::test]
    async fn test_login_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid123", "full_name": "Test User"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "csrf_token_value"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "email": "admin@example.com", "roles": ["System Manager"]}
            })))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let s = client.session.unwrap();
        assert_eq!(s.sid.expose_secret(), "sid123");
        assert_eq!(s.csrf_token.expose_secret(), "csrf_token_value");
        assert_eq!(s.full_name, Some("Test User".to_string()));
        assert_eq!(s.roles, vec!["System Manager"]);
    }

    #[tokio::test]
    async fn test_login_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::RateLimited)));
    }

    #[tokio::test]
    async fn test_login_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::Auth(_))));
    }

    #[tokio::test]
    async fn test_login_csrf_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::Csrf(_))));
    }

    #[tokio::test]
    async fn test_login_user_info_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"message": "csrf"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::Auth(_))));
    }

    #[tokio::test]
    async fn test_login_sitename_mismatch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"message": "csrf"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "roles": []}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"some html "sitename": "wrongsite" more"#),
            )
            .mount(&server)
            .await;

        let config = ClientConfig {
            base_url: Url::parse(&server.uri()).unwrap(),
            auth_mode: session_auth("user", "pass"),
            expected_sitename: Some("expectedsite".into()),
            required_roles: vec![],
            timeout_secs: 5,
            max_retries: 1,
            user_agent: "test".into(),
            insecure_ssl: true,
        };
        let mut client = RjssClient::new(config).unwrap();
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::SitenameMismatch { .. })));
    }

    #[tokio::test]
    async fn test_login_missing_required_roles() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "sid"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"message": "csrf"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "roles": ["Guest"]}
            })))
            .mount(&server)
            .await;

        let config = ClientConfig {
            base_url: Url::parse(&server.uri()).unwrap(),
            auth_mode: session_auth("user", "pass"),
            expected_sitename: None,
            required_roles: vec!["System Manager".into()],
            timeout_secs: 5,
            max_retries: 1,
            user_agent: "test".into(),
            insecure_ssl: true,
        };
        let mut client = RjssClient::new(config).unwrap();
        let result = client.authenticate().await;
        assert!(matches!(result, Err(JuraganError::Permission(_))));
    }

    #[tokio::test]
    async fn test_logout_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/logout"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.session = Some(SessionInfo {
            sid: SecretString::new(Box::from("sid".to_string())),
            csrf_token: SecretString::new(Box::from("csrf".to_string())),
            full_name: None,
            sitename: "".into(),
            roles: vec![],
        });
        client.credentials = Some((
            SecretString::new(Box::from("user".to_string())),
            SecretString::new(Box::from("pass".to_string())),
        ));

        client.logout().await.unwrap();
        assert!(client.session.is_none());
        assert!(client.credentials.is_none());
    }

    #[tokio::test]
    async fn test_logout_not_authenticated() {
        let server = MockServer::start().await;
        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.logout().await;
        assert!(matches!(result, Err(JuraganError::NotAuthenticated)));
    }

    #[tokio::test]
    async fn test_logout_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/method/logout"))
            .respond_with(ResponseTemplate::new(500).set_body_string("error"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.session = Some(SessionInfo {
            sid: SecretString::new(Box::from("sid".to_string())),
            csrf_token: SecretString::new(Box::from("csrf".to_string())),
            full_name: None,
            sitename: "".into(),
            roles: vec![],
        });

        let result = client.logout().await;
        assert!(matches!(result, Err(JuraganError::Http { .. })));
    }

    #[tokio::test]
    async fn test_ensure_session_valid() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "roles": []}
            })))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.session = Some(SessionInfo {
            sid: SecretString::new(Box::from("sid".to_string())),
            csrf_token: SecretString::new(Box::from("csrf".to_string())),
            full_name: None,
            sitename: "".into(),
            roles: vec![],
        });

        let result = client.ensure_session().await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn test_ensure_session_expired_triggers_relogin() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/method/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"sid": "new_sid"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_csrf_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"message": "new_csrf"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/method/frappe.auth.get_logged_user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {"name": "admin", "roles": []}
            })))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.session = Some(SessionInfo {
            sid: SecretString::new(Box::from("old_sid".to_string())),
            csrf_token: SecretString::new(Box::from("old_csrf".to_string())),
            full_name: None,
            sitename: "".into(),
            roles: vec![],
        });
        client.credentials = Some((
            SecretString::new(Box::from("user".to_string())),
            SecretString::new(Box::from("pass".to_string())),
        ));

        client.ensure_session().await.unwrap();
        assert_eq!(
            client.session.as_ref().unwrap().sid.expose_secret(),
            "new_sid"
        );
    }
    #[tokio::test]
    async fn test_authenticated_get_success() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("GET"))
            .and(path("/api/resource/test"))
            .respond_with(ResponseTemplate::new(200).set_body_string("response body"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let body = client
            .authenticated_get("/api/resource/test")
            .await
            .unwrap();
        assert_eq!(body, "response body");
    }

    #[tokio::test]
    async fn test_authenticated_get_unauthorized() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("GET"))
            .and(path("/api/resource/test"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let result = client.authenticated_get("/api/resource/test").await;
        assert!(matches!(result, Err(JuraganError::Auth(_))));
    }

    #[tokio::test]
    async fn test_authenticated_get_server_error_retry() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("GET"))
            .and(path("/api/resource/test"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/resource/test"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(1)
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let body = client
            .authenticated_get("/api/resource/test")
            .await
            .unwrap();
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn test_authenticated_post_with_csrf() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("POST"))
            .and(path("/api/resource/test"))
            .and(wiremock::matchers::header("X-Frappe-CSRF-Token", "csrf"))
            .respond_with(ResponseTemplate::new(200).set_body_string("created"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let body = client
            .authenticated_post("/api/resource/test", r#"{"key":"value"}"#)
            .await
            .unwrap();
        assert_eq!(body, "created");
    }

    #[tokio::test]
    async fn test_authenticated_put_with_csrf() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("PUT"))
            .and(path("/api/resource/test"))
            .and(wiremock::matchers::header("X-Frappe-CSRF-Token", "csrf"))
            .respond_with(ResponseTemplate::new(200).set_body_string("updated"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let body = client
            .authenticated_put("/api/resource/test", r#"{"key":"value"}"#)
            .await
            .unwrap();
        assert_eq!(body, "updated");
    }

    #[tokio::test]
    async fn test_authenticated_delete_with_csrf() {
        let server = MockServer::start().await;
        mock_authenticated_requests(&server).await;
        Mock::given(method("DELETE"))
            .and(path("/api/resource/test"))
            .and(wiremock::matchers::header("X-Frappe-CSRF-Token", "csrf"))
            .respond_with(ResponseTemplate::new(200).set_body_string("deleted"))
            .mount(&server)
            .await;

        let mut client = setup_test_client(&server, session_auth("user", "pass")).await;
        client.authenticate().await.unwrap();

        let body = client
            .authenticated_delete("/api/resource/test")
            .await
            .unwrap();
        assert_eq!(body, "deleted");
    }

    #[tokio::test]
    async fn test_authenticated_request_path_traversal_error() {
        let server = MockServer::start().await;
        let client = setup_test_client(&server, session_auth("user", "pass")).await;
        let result = client.authenticated_get("/../etc/passwd").await;
        assert!(matches!(result, Err(JuraganError::Validation(_))));
    }

    #[tokio::test]
    async fn test_token_mode_authenticated_request_has_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/resource/test"))
            .and(wiremock::matchers::header(
                "Authorization",
                "token key:secret",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let config = ClientConfig {
            base_url: Url::parse(&server.uri()).unwrap(),
            auth_mode: AuthMode::Token {
                api_key: "key".into(),
                api_secret: SecretString::new(Box::from("secret".to_string())),
            },
            expected_sitename: None,
            required_roles: vec![],
            timeout_secs: 5,
            max_retries: 1,
            user_agent: "test".into(),
            insecure_ssl: true,
        };
        let client = RjssClient::new(config).unwrap();
        let body = client
            .authenticated_get("/api/resource/test")
            .await
            .unwrap();
        assert_eq!(body, "ok");
    }

    #[test]
    fn test_build_join_url_traversal() {
        let config = ClientConfig {
            base_url: Url::parse("https://example.com").unwrap(),
            auth_mode: AuthMode::Token {
                api_key: "k".into(),
                api_secret: SecretString::new(Box::from("s".to_string())),
            },
            expected_sitename: None,
            required_roles: vec![],
            timeout_secs: 1,
            max_retries: 0,
            user_agent: "t".into(),
            insecure_ssl: false,
        };
        let client = RjssClient::new(config).unwrap();
        assert!(client.build_join_url("/valid/path").is_ok());
        assert!(client.build_join_url("/../etc").is_err());
    }
}
