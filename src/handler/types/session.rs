pub struct SessionInfo {
    pub sid: secrecy::SecretString,
    pub csrf_token: secrecy::SecretString,
    pub full_name: Option<String>,
    pub sitename: String,
    pub roles: Vec<String>,
}
