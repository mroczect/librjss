use async_trait::async_trait;
use librjss::handler::error::AuthError;
use librjss::handler::types::{Credentials, UserInfo, UserProvider};

pub struct HardcodedUserProvider;

#[async_trait]
impl UserProvider for HardcodedUserProvider {
    async fn authenticate(&self, creds: &Credentials) -> Result<UserInfo, AuthError> {
        if creds.username == "admin" && creds.password == "secret" {
            Ok(UserInfo {
                user_id: "admin".into(),
                extra: serde_json::json!({"role": "superuser"}),
            })
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError> {
        if user_id == "admin" {
            Ok(Some(UserInfo {
                user_id: "admin".into(),
                extra: serde_json::json!({"role": "superuser"}),
            }))
        } else {
            Ok(None)
        }
    }
}
