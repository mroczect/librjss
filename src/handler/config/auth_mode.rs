/// Authentication mode for JSS client.
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// Session-based authentication using email and password.
    Session {
        email: secrecy::SecretString,
        password: secrecy::SecretString,
    },
    /// Token-based authentication using API key and secret.
    Token {
        api_key: String,
        api_secret: secrecy::SecretString,
    },
}
