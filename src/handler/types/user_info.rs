use serde::Deserialize;

/// Response from the `get_logged_user` endpoint.
#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub message: UserInfoMessage,
}

/// Message payload inside a user info response.
#[derive(Debug, Deserialize)]
pub struct UserInfoMessage {
    pub name: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
}
