use crate::handler::error::JssError;
use reqwest::Url;

pub trait AuthEndpoints {
    fn login_url(base: &Url) -> Result<Url, JssError> {
        base.join("/api/method/login")
            .map_err(|e| JssError::Parse(format!("Invalid login URL: {e}")))
    }
    fn logout_url(base: &Url) -> Result<Url, JssError> {
        base.join("/api/method/logout")
            .map_err(|e| JssError::Parse(format!("Invalid logout URL: {e}")))
    }
    fn csrf_token_url(base: &Url) -> Result<Url, JssError> {
        base.join("/api/method/frappe.auth.get_csrf_token")
            .map_err(|e| JssError::Parse(format!("Invalid CSRF URL: {e}")))
    }
    fn get_logged_user_url(base: &Url) -> Result<Url, JssError> {
        base.join("/api/method/frappe.auth.get_logged_user")
            .map_err(|e| JssError::Parse(format!("Invalid get_logged_user URL: {e}")))
    }
    fn app_page_url(base: &Url) -> Result<Url, JssError> {
        base.join("/app")
            .map_err(|e| JssError::Parse(format!("Invalid /app URL: {e}")))
    }
}
