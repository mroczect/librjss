use crate::api::auth::AuthEndpoints;
use crate::client::RjssClient;
use crate::handler::error::JuraganError;
use crate::handler::types::boot::FrappeBoot;
use regex::Regex;
use scraper::{Html, Selector};
use secrecy::SecretString;
use tracing::error;

pub(crate) async fn fetch_app_page(client: &RjssClient) -> Result<String, JuraganError> {
    let app_url = RjssClient::app_page_url(&client.config.base_url);
    let resp = client.http.get(app_url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    error!(trace_id = client.trace_id, %status, %body, "Raw /app response");
    if !status.is_success() {
        return Err(JuraganError::Auth("Failed to load /app".into()));
    }
    Ok(body)
}

pub(crate) fn extract_app_data(
    html: &str,
) -> Result<(SecretString, FrappeBoot), JuraganError> {
    let document = Html::parse_document(html);

    let selector = Selector::parse("script").unwrap();
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

    let re = Regex::new(r"frappe\.boot\s*=\s*(\{.*?\});\s*\n")
        .map_err(|_| JuraganError::Parse("Regex compilation error".into()))?;
    let caps = re.captures(html).ok_or(JuraganError::Parse(
        "Could not find frappe.boot object in /app".into(),
    ))?;
    let boot_json = caps.get(1).unwrap().as_str();

    let boot: FrappeBoot = serde_json::from_str(boot_json)
        .map_err(|e| JuraganError::Parse(format!("Failed to parse frappe.boot: {e}")))?;

    Ok((SecretString::new(Box::from(csrf_token)), boot))
}
