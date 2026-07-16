use actix_web::HttpResponse;
use librjss::handler::error::AuthError;

pub fn auth_error_to_response(e: AuthError) -> HttpResponse {
    let status = actix_web::http::StatusCode::from_u16(e.status_code().as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    HttpResponse::build(status).json(e.to_json_body())
}
