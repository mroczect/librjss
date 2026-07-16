use crate::api::AuthManager;
use crate::api::auth::{cookies, session};
use crate::error::AuthError;
use crate::handler::types::{HttpResponse, SessionId};

pub async fn handle_logout(
    manager: &AuthManager,
    session_id: Option<&SessionId>,
) -> Result<HttpResponse, AuthError> {
    if let Some(sid) = session_id {
        if let Err(e) = session::destroy_session(&*manager.session_store, sid).await {
            tracing::error!(session_id = %sid, error = %e, "Failed to destroy session");
        } else {
            tracing::info!(session_id = %sid, "Session destroyed");
        }
    }

    let removal_cookie = cookies::create_removal_cookie(&manager.config);

    let mut response = if let Some(ref redirect_url) = manager.config.logout_redirect_url {
        HttpResponse::new(
            http::StatusCode::FOUND,
            format!("Redirecting to {}", redirect_url),
        )
        .with_header("Location".to_string(), redirect_url.clone())
    } else {
        HttpResponse::json(
            http::StatusCode::OK,
            serde_json::json!({"status": "logged_out"}),
        )
    };

    response = response.with_header("Set-Cookie".to_string(), removal_cookie.to_string());

    Ok(response)
}
