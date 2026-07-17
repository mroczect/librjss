#[derive(Debug, Clone)]
pub enum AuthMode {
    Session {
        email: secrecy::SecretString,
        password: secrecy::SecretString,
    },
    Token {
        api_key: String,
        api_secret: secrecy::SecretString,
    },
}
