use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub message: UserInfoMessage,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoMessage {
    pub name: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
}
