use crate::error::auth_error_to_response;
use crate::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};
use librjss::api::auth::cookies;
use tracing::info;

pub async fn logout(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let cookie_header = req
        .headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let session_id =
        cookies::parse_session_id_from_cookie(cookie_header, &state.auth_manager.config);

    if let Some(sid) = &session_id {
        if let Ok(info) = state.auth_manager.validate_session(sid).await {
            if let Some(ext) = &state.external_session_store {
                ext.remove(&info.user_id);
            }
        }
    }

    match state.auth_manager.logout(session_id.as_ref()).await {
        Ok(resp) => {
            info!("Logout successful");
            super::login::convert_librjss_response(resp)
        }
        Err(e) => {
            info!("Logout error: {}", e);
            auth_error_to_response(e)
        }
    }
}
