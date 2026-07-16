use actix_web::{HttpResponse, web};
use serde::Deserialize;
use tracing::info;

use crate::error::auth_error_to_response;
use crate::state::AppState;
use librjss::handler::types::Credentials;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(state: web::Data<AppState>, body: web::Json<LoginRequest>) -> HttpResponse {
    let creds = Credentials {
        username: body.username.clone(),
        password: body.password.clone(),
    };

    match state.auth_manager.login(creds).await {
        Ok(resp) => {
            info!("User '{}' logged in", body.username);
            convert_librjss_response(resp)
        }
        Err(e) => {
            info!("Login failed for '{}': {}", body.username, e);
            auth_error_to_response(e)
        }
    }
}

pub fn convert_librjss_response(resp: librjss::handler::types::HttpResponse) -> HttpResponse {
    let mut builder = HttpResponse::build(
        actix_web::http::StatusCode::from_u16(resp.status.as_u16()).expect("valid status code"),
    );
    for (key, value) in resp.headers {
        builder.insert_header((key.as_str(), value.as_str()));
    }
    builder.body(resp.body)
}
