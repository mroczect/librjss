use serde::Deserialize;

/// Response from the login API endpoint.
#[derive(Debug, Deserialize)]
pub struct LoginApiResponse {
    pub message: LoginApiMessage,
}

/// The message payload inside a login response.
#[derive(Debug, Deserialize)]
pub struct LoginApiMessage {
    pub sid: String,
    #[serde(default)]
    pub full_name: Option<String>,
}
