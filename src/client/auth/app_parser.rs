use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JssError;
use crate::handler::types::boot::FrappeBoot;
use regex::Regex;
use scraper::{Html, Selector};
use secrecy::SecretString;
use tracing::trace;

pub(crate) async fn fetch_app_page(client: &RjssClient) -> Result<String, JssError> {
    let app_url = RjssClient::app_page_url(&client.config.base_url)?;
    let resp = client.http.get(app_url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    trace!(trace_id = client.trace_id, %status, "Fetched /app page ({} bytes)", body.len());
    if !status.is_success() {
        return Err(JssError::Auth("Failed to load /app".into()));
    }
    Ok(body)
}

pub(crate) fn extract_app_data(html: &str) -> Result<(SecretString, FrappeBoot), JssError> {
    let document = Html::parse_document(html);

    let selector = Selector::parse("script")
        .map_err(|_| JssError::Parse("Failed to parse CSS selector".into()))?;
    let mut csrf_token = String::new();
    for script in document.select(&selector) {
        let text = script.inner_html();
        if let Some(pos) = text.find("frappe.csrf_token") {
            if let Some(start) = text[pos..].find('"') {
                let start = pos + start + 1;
                if let Some(end) = text[start..].find('"') {
                    csrf_token = text[start..start + end].to_string();
                    break;
                }
            }
        }
    }

    let re = Regex::new(r"(?s)frappe\.boot\s*=\s*(\{.*?\});\s*\n")
        .map_err(|_| JssError::Parse("Regex compilation error".into()))?;
    let caps = re.captures(html).ok_or(JssError::Parse(
        "Could not find frappe.boot object in /app".into(),
    ))?;
    let boot_json = caps
        .get(1)
        .ok_or(JssError::Parse("Regex capture group missing".into()))?
        .as_str();

    let boot: FrappeBoot = serde_json::from_str(boot_json)
        .map_err(|e| JssError::Parse(format!("Failed to parse frappe.boot: {e}")))?;

    Ok((SecretString::new(Box::from(csrf_token)), boot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn make_html(csrf: &str, boot_json: &str) -> String {
        format!(
            r#"<html><head></head><body>
            <script>frappe.csrf_token = "{csrf}";</script>
            <script>
            frappe.boot = {boot_json};
            </script>
            </body></html>"#
        )
    }

    #[test]
    fn test_extract_csrf_and_boot_valid() {
        let boot_json = r#"{
            "sitename":"test",
            "user":{"name":"dev","full_name":"Dev User","roles":["Admin"]},
            "csrf_token":"abc123",
            "versions":{},
            "lang_dict":{},
            "sidebar_pages":{"pages":[],"has_access":false,"has_create_access":false},
            "navbar_settings":null,
            "developer_mode":0,
            "read_only":false
        }"#;
        let html = make_html("abc123", boot_json);
        let (csrf, boot) = extract_app_data(&html).expect("Valid HTML");
        assert_eq!(csrf.expose_secret(), "abc123");
        assert_eq!(boot.sitename, "test");
        assert_eq!(boot.user.name, "dev");
        assert_eq!(boot.user.roles, vec!["Admin"]);
        assert_eq!(boot.developer_mode, 0);
        assert!(!boot.read_only);
    }

    #[test]
    fn test_extract_boot_from_real_html() {
        let html = include_str!("../../../tests/fixtures/real_app_page.html");
        let (csrf, boot) = extract_app_data(html).expect("Failed to parse real HTML");
        assert!(!csrf.expose_secret().is_empty());
        assert!(!boot.sitename.is_empty());
        assert!(!boot.user.name.is_empty());
        assert!(!boot.versions.is_empty());
    }

    #[test]
    fn test_extract_csrf_no_token() {
        let html = r#"<html><script>frappe.csrf_token = ;</script>
        <script>
        frappe.boot = {"sitename":"x","user":{"name":"u","full_name":"","roles":[]},"versions":{},"lang_dict":{},"sidebar_pages":{"pages":[],"has_access":false,"has_create_access":false},"navbar_settings":null,"developer_mode":0,"read_only":false};
        </script></html>"#;
        let (csrf, _) = extract_app_data(html).expect("No token should be empty string");
        assert!(csrf.expose_secret().is_empty());
    }

    #[test]
    fn test_extract_missing_boot() {
        let html = r#"<html><script>frappe.csrf_token = "token";</script></html>"#;
        let result = extract_app_data(html);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), JssError::Parse(_)));
    }

    #[test]
    fn test_extract_invalid_boot_json() {
        let html = r#"<html><script>frappe.csrf_token = "token";</script>
        <script>
        frappe.boot = {invalid};
        </script></html>"#;
        let result = extract_app_data(html);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), JssError::Parse(_)));
    }

    #[test]
    fn test_extract_boot_with_extra_fields() {
        let boot_json = r#"{
            "sitename":"extra",
            "user":{"name":"u","full_name":"F","roles":[]},
            "versions":{"frappe":"16"},
            "lang_dict":{"en":"English"},
            "sidebar_pages":{"pages":[],"has_access":true,"has_create_access":false},
            "navbar_settings":null,
            "developer_mode":1,
            "read_only":true
        }"#;
        let html = make_html("tok", boot_json);
        let (_, boot) = extract_app_data(&html).expect("Extra fields");
        assert_eq!(boot.versions.get("frappe").unwrap(), "16");
        assert_eq!(boot.lang_dict.get("en").unwrap(), "English");
        assert_eq!(boot.developer_mode, 1);
        assert!(boot.read_only);
    }
}
