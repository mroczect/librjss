use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use tracing::warn;

const MAX_REAUTH_ATTEMPTS: u32 = 3;

pub(crate) async fn ensure_session(client: &mut RjssClient) -> Result<(), JssError> {
    let mut attempts = 0;
    loop {
        if client.session.is_none() {
            if let Some((email, password)) = client.credentials.clone() {
                if attempts >= MAX_REAUTH_ATTEMPTS {
                    return Err(JssError::Auth(
                        "Re-authentication failed after maximum attempts".into(),
                    ));
                }
                super::login::login_with_credentials(client, email, password).await?;
                return Ok(());
            } else {
                return Err(JssError::NotAuthenticated);
            }
        } else {
            let url = RjssClient::get_logged_user_url(&client.config.base_url)?;
            let resp = client.http.get(url).send().await?;
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                warn!(
                    trace_id = client.trace_id,
                    "Session expired, will re-login (attempt {})",
                    attempts + 1
                );
                client.session = None;
                attempts += 1;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            } else {
                return Ok(());
            }
        }
    }
}
