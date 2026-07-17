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
use tracing::{debug, error, info, instrument};

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
            return Err(JuraganError::RateLimited);
        }
        if status != StatusCode::OK {
            let body = resp.text().await.unwrap_or_default();
            error!(trace_id = self.trace_id, %status, %body, "Login failed");
            return Err(JuraganError::Auth(format!("Login failed: HTTP {status}")));
        }

        let body_text = resp.text().await?;
        debug!(
            trace_id = self.trace_id,
            body_hash = format!("{:x}", Sha256::digest(body_text.as_bytes())),
            "Login response received"
        );

        let login_resp: LoginApiResponse = serde_json::from_str(&body_text)
            .map_err(|e| JuraganError::Parse(format!("Failed to parse login response: {e}")))?;

        let sid = SecretString::new(Box::from(login_resp.message.sid));
        let csrf_token = self.obtain_csrf_token().await?;
        let user_info = self.fetch_user_info().await?;

        if let Some(expected_sitename) = &self.config.expected_sitename {
            let actual = self.fetch_sitename_from_app().await?;
            if expected_sitename != &actual {
                return Err(JuraganError::SitenameMismatch {
                    expected: expected_sitename.clone(),
                    actual,
                });
            }
            info!(trace_id = self.trace_id, sitename = %actual, "Sitename verified");
        }

        if !self.config.required_roles.is_empty() {
            let user_roles = &user_info.message.roles;
            let has_role = self
                .config
                .required_roles
                .iter()
                .any(|r| user_roles.contains(r));
            if !has_role {
                return Err(JuraganError::Permission(format!(
                    "Missing one of required roles: {:?}",
                    self.config.required_roles
                )));
            }
        }

        self.session = Some(SessionInfo {
            sid,
            csrf_token,
            full_name: login_resp.message.full_name,
            sitename: self.config.expected_sitename.clone().unwrap_or_default(),
            roles: user_info.message.roles,
        });

        info!(trace_id = self.trace_id, email_hash = %email_hash, "Login successful");
        Ok(())
    }

    async fn obtain_csrf_token(&self) -> Result<SecretString, JuraganError> {
        let csrf_url = Self::csrf_token_url(&self.config.base_url);
        let resp = self.http.get(csrf_url).send().await?;
        if resp.status().is_success() {
            let body = resp.text().await?;
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(token) = json.get("message").and_then(|v| v.as_str()) {
                    return Ok(SecretString::new(Box::from(token.to_owned())));
                }
            }
        }
        Err(JuraganError::Csrf("Unable to obtain CSRF token".into()))
    }

    async fn fetch_user_info(&self) -> Result<crate::handler::types::UserInfo, JuraganError> {
        let url = Self::get_logged_user_url(&self.config.base_url);
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(JuraganError::Auth("Failed to verify session".into()));
        }
        let body = resp.text().await?;
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
            Err(JuraganError::Parse("Sitename not found in /app".into()))
        }
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

    fn build_join_url(&self, path: &str) -> Result<reqwest::Url, JuraganError> {
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
                    self.session = None;
                } else {
                    return Ok(());
                }
            }
        }
    }
}
