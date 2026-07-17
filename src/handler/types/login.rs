use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LoginApiResponse {
    pub message: LoginApiMessage,
}

#[derive(Debug, Deserialize)]
pub struct LoginApiMessage {
    pub sid: String,
    #[serde(default)]
    pub full_name: Option<String>,
}
