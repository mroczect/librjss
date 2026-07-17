use reqwest::Url;
use crate::handler::config::auth_mode::AuthMode;

/// Configuration for connecting to a JSS instance.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub base_url: Url,
    pub auth_mode: AuthMode,
    pub expected_sitename: Option<String>,
    pub required_roles: Vec<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub user_agent: String,
    pub insecure_ssl: bool,
}
