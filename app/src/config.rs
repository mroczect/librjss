use librjss::handler::config::AuthConfig;

pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub allowed_origins: Vec<String>,
    pub auth: AuthConfig,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse()
            .expect("SERVER_PORT must be a valid u16");

        let allowed_origins: Vec<String> = std::env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let auth =
            AuthConfig::from_env().expect("Failed to load auth configuration from environment");

        Self {
            host,
            port,
            allowed_origins,
            auth,
        }
    }
}
