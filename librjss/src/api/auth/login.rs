use crate::api::AuthManager;
use crate::api::auth::{cookies, session};
use crate::error::AuthError;
use crate::handler::types::{Credentials, HttpResponse};

pub async fn handle_login(
    manager: &AuthManager,
    credentials: Credentials,
) -> Result<HttpResponse, AuthError> {
    if credentials.username.trim().is_empty() || credentials.password.is_empty() {
        return Err(AuthError::InvalidCredentials);
    }

    let user_info = manager
        .user_provider
        .authenticate(&credentials)
        .await
        .map_err(|e| {
            tracing::warn!(username = %credentials.username, error = %e, "Login failed");
            e
        })?;

    tracing::info!(user_id = %user_info.user_id, "User authenticated");

    let (session_id, _) =
        session::create_session(&*manager.session_store, &manager.config, &user_info)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to create session");
                AuthError::Internal("session creation failed".into())
            })?;

    let cookie = cookies::create_session_cookie(&session_id, &manager.config);

    let json_body = serde_json::json!({
        "status": "success",
        "user_id": user_info.user_id
    });

    let response = HttpResponse::json(http::StatusCode::OK, json_body)
        .with_header("Set-Cookie".to_string(), cookie.to_string());

    Ok(response)
}
