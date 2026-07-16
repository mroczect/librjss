use crate::error::auth_error_to_response;
use crate::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};
use librjss::api::auth::cookies;
use tracing::info;

pub async fn check(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let cookie_header = req
        .headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let session_id =
        cookies::parse_session_id_from_cookie(cookie_header, &state.auth_manager.config);

    match session_id {
        Some(sid) => match state.auth_manager.validate_session(&sid).await {
            Ok(info) => {
                info!("Session valid for user '{}'", info.user_id);
                HttpResponse::Ok().json(info)
            }
            Err(e) => {
                info!("Session validation failed: {}", e);
                auth_error_to_response(e)
            }
        },
        None => {
            info!("No session cookie in request");
            HttpResponse::Unauthorized().json(serde_json::json!({"error": "no session cookie"}))
        }
    }
}
