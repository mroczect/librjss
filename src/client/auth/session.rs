use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use tracing::warn;

pub(crate) async fn ensure_session(client: &mut RjssClient) -> Result<(), JssError> {
    loop {
        if client.session.is_none() {
            if let Some((email, password)) = client.credentials.clone() {
                super::login::login_with_credentials(client, email, password).await?;
                return Ok(());
            } else {
                return Err(JssError::NotAuthenticated);
            }
        } else {
            let url = RjssClient::get_logged_user_url(&client.config.base_url)?;
            let resp = client.http.get(url).send().await?;
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                warn!(trace_id = client.trace_id, "Session expired, will re-login");
                client.session = None;
            } else {
                return Ok(());
            }
        }
    }
}
