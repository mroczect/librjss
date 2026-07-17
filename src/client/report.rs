use crate::client::RjssClient;
use crate::handler::error::JssError;
use std::collections::HashMap;
use tracing::instrument;

pub struct ReportBuilder<'a> {
    client: &'a RjssClient,
    report_name: String,
    filters: HashMap<String, String>,
}

impl<'a> ReportBuilder<'a> {
    pub(crate) fn new(client: &'a RjssClient, report_name: &str) -> Self {
        ReportBuilder {
            client,
            report_name: report_name.to_string(),
            filters: HashMap::new(),
        }
    }

    pub fn add_filter(mut self, field: &str, value: &str) -> Self {
        self.filters.insert(field.to_string(), value.to_string());
        self
    }

    #[instrument(skip(self), fields(report = %self.report_name))]
    pub async fn run_raw(&self) -> Result<String, JssError> {
        let body = serde_json::json!({
            "report_name": self.report_name,
            "filters": self.filters,
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| JssError::Parse(format!("Failed to serialize report data: {e}")))?;

        self.client
            .authenticated_post("/api/method/frappe.desk.query_report.run", &body_str)
            .await
    }

    pub async fn run<T: serde::de::DeserializeOwned>(&self) -> Result<T, JssError> {
        let body = self.run_raw().await?;
        serde_json::from_str(&body)
            .map_err(|e| JssError::Parse(format!("Failed to parse report result: {e}")))
    }
}

impl RjssClient {
    pub fn report(&self, name: &str) -> ReportBuilder<'_> {
        ReportBuilder::new(self, name)
    }
}
