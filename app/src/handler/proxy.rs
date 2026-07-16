use crate::state::AppState;
use actix_web::{HttpRequest, HttpResponse, web};
use librjss::api::auth::cookies;

pub async fn proxy_collection(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let cookie_header = req
        .headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let session_id =
        cookies::parse_session_id_from_cookie(cookie_header, &state.auth_manager.config);

    let session_id = match session_id {
        Some(sid) => sid,
        None => {
            return HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "no session cookie"}));
        }
    };

    let user_info = match state.auth_manager.validate_session(&session_id).await {
        Ok(info) => info,
        Err(e) => return HttpResponse::Unauthorized().json(e.to_json_body()),
    };

    let external_cookie = match &state.external_session_store {
        Some(ext) => match ext.get(&user_info.user_id) {
            Some(c) => c,
            None => {
                return HttpResponse::Unauthorized()
                    .json(serde_json::json!({"error": "no external session"}));
            }
        },
        None => {
            return HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "proxy only available in prod mode"}));
        }
    };

    let path = req.match_info().get("tail").unwrap_or("/");

    match state
        .http_client
        .get(format!("https://app.juragansejati.biz.id{}", path))
        .header("Cookie", external_cookie)
        .send()
        .await
    {
        Ok(res) => {
            let status = actix_web::http::StatusCode::from_u16(res.status().as_u16())
                .unwrap_or(actix_web::http::StatusCode::BAD_GATEWAY);
            let body = res.text().await.unwrap_or_default();
            HttpResponse::build(status).body(body)
        }
        Err(e) => HttpResponse::BadGateway()
            .json(serde_json::json!({"error": format!("Proxy error: {}", e)})),
    }
}
