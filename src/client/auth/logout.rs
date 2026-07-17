use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use secrecy::ExposeSecret;
use tracing::{error, info, instrument};

#[instrument(skip(client), fields(trace_id = client.trace_id))]
pub(crate) async fn logout(client: &mut RjssClient) -> Result<(), JssError> {
    let session = client.session.as_ref().ok_or(JssError::NotAuthenticated)?;
    let csrf = session.csrf_token.expose_secret();
    let logout_url = RjssClient::logout_url(&client.config.base_url);
    let resp = client
        .http
        .post(logout_url)
        .header("X-Frappe-CSRF-Token", csrf)
        .send()
        .await?;
    let status = resp.status();
    if status.is_success() {
        info!(trace_id = client.trace_id, "Logout successful");
        client.session = None;
        client.credentials = None;
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        error!(trace_id = client.trace_id, %status, %body, "Logout failed");
        Err(JssError::Http { status, body })
    }
}
