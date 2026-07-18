use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use secrecy::ExposeSecret;
use tracing::{info, instrument, warn};

#[instrument(skip(client), fields(trace_id = client.trace_id))]
pub(crate) async fn logout(client: &mut RjssClient) -> Result<(), JssError> {
    let session = client.session.as_ref().ok_or(JssError::NotAuthenticated)?;
    let csrf = session.csrf_token.expose_secret();
    let logout_url = RjssClient::logout_url(&client.config.base_url)?;
    let mut req = client.http.post(logout_url);
    if !csrf.is_empty() {
        req = req.header("X-Frappe-CSRF-Token", csrf);
    }
    let resp = req.send().await?;
    let status = resp.status();
    if status.is_success() {
        info!(trace_id = client.trace_id, "Logout successful");
        client.session = None;
        client.credentials = None;
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        warn!(trace_id = client.trace_id, %status, "Logout failed");
        Err(JssError::Http { status, body })
    }
}
