use crate::client::RjssClient;
use crate::handler::error::JssError;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use tracing::instrument;

pub struct ResourceBuilder<'a> {
    client: &'a RjssClient,
    doctype: String,
    filters: Vec<(String, String, String)>,
    fields: Option<Vec<String>>,
    order_by: Option<String>,
    limit: Option<u32>,
    limit_start: Option<u32>,
}

impl<'a> ResourceBuilder<'a> {
    pub(crate) fn new(client: &'a RjssClient, doctype: &str) -> Self {
        ResourceBuilder {
            client,
            doctype: doctype.to_string(),
            filters: Vec::new(),
            fields: None,
            order_by: None,
            limit: None,
            limit_start: None,
        }
    }

    pub fn filter(mut self, field: &str, operator: &str, value: &str) -> Self {
        self.filters
            .push((field.to_string(), operator.to_string(), value.to_string()));
        self
    }

    pub fn fields(mut self, fields: Vec<&str>) -> Self {
        self.fields = Some(fields.into_iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn order_by(mut self, order: &str) -> Self {
        self.order_by = Some(order.to_string());
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        if limit == 0 {
            tracing::warn!("limit set to 0, using default 200");
            self.limit = Some(200);
        } else {
            self.limit = Some(limit);
        }
        self
    }

    pub fn limit_start(mut self, start: u32) -> Self {
        self.limit_start = Some(start);
        self
    }

    #[instrument(skip(self), fields(doctype = %self.doctype))]
    pub async fn execute_raw(&self) -> Result<String, JssError> {
        let mut path = format!("/api/resource/{}", self.doctype);
        let mut params = Vec::new();

        if !self.filters.is_empty() {
            let filters_json: Vec<[String; 3]> = self
                .filters
                .iter()
                .map(|(f, op, v)| [f.clone(), op.clone(), v.clone()])
                .collect();
            let filters_str = serde_json::to_string(&filters_json)
                .map_err(|e| JssError::Parse(format!("Failed to serialize filters: {e}")))?;
            params.push(format!("filters={}", urlencoding::encode(&filters_str)));
        }

        if let Some(ref fields) = self.fields {
            params.push(format!(
                "fields={}",
                urlencoding::encode(&serde_json::to_string(fields).unwrap_or_default())
            ));
        }

        if let Some(ref order) = self.order_by {
            params.push(format!("order_by={}", urlencoding::encode(order)));
        }

        if let Some(limit) = self.limit {
            params.push(format!("limit_page_length={limit}"));
        }

        if let Some(start) = self.limit_start {
            params.push(format!("limit_start={start}"));
        }

        if !params.is_empty() {
            path = format!("{path}?{}", params.join("&"));
        }

        self.client.authenticated_get(&path).await
    }

    pub async fn execute<T: DeserializeOwned>(&self) -> Result<T, JssError> {
        let body = self.execute_raw().await?;
        serde_json::from_str(&body)
            .map_err(|e| JssError::Parse(format!("Failed to parse response: {e}")))
    }

    pub async fn all<T: DeserializeOwned>(&self) -> Result<Vec<T>, JssError> {
        let page_size = 200u32;
        let mut all_data = Vec::new();
        let mut start = 0u32;

        loop {
            let builder = ResourceBuilder {
                client: self.client,
                doctype: self.doctype.clone(),
                filters: self.filters.clone(),
                fields: self.fields.clone(),
                order_by: self.order_by.clone(),
                limit: Some(page_size),
                limit_start: Some(start),
            };

            let response: serde_json::Value = builder.execute().await?;
            let data = response["data"].as_array().cloned().unwrap_or_default();
            if data.is_empty() {
                break;
            }

            all_data.extend(data);
            start += page_size;
        }

        let all_json = serde_json::Value::Array(all_data);
        serde_json::from_value(all_json)
            .map_err(|e| JssError::Parse(format!("Failed to deserialize all data: {e}")))
    }
}

impl RjssClient {
    pub fn doctype(&self, name: &str) -> ResourceBuilder<'_> {
        ResourceBuilder::new(self, name)
    }

    pub async fn get_doc(&self, doctype: &str, name: &str) -> Result<String, JssError> {
        let path = format!("/api/resource/{doctype}/{name}");
        self.authenticated_get(&path).await
    }

    pub async fn create_doc(
        &self,
        doctype: &str,
        data: &impl Serialize,
    ) -> Result<String, JssError> {
        let path = format!("/api/resource/{doctype}");
        let body = serde_json::to_string(data)
            .map_err(|e| JssError::Parse(format!("Failed to serialize data: {e}")))?;
        self.authenticated_post(&path, &body).await
    }

    pub async fn update_doc(
        &self,
        doctype: &str,
        name: &str,
        data: &impl Serialize,
    ) -> Result<String, JssError> {
        let path = format!("/api/resource/{doctype}/{name}");
        let body = serde_json::to_string(data)
            .map_err(|e| JssError::Parse(format!("Failed to serialize data: {e}")))?;
        self.authenticated_put(&path, &body).await
    }

    pub async fn delete_doc(&self, doctype: &str, name: &str) -> Result<String, JssError> {
        let path = format!("/api/resource/{doctype}/{name}");
        self.authenticated_delete(&path).await
    }

    pub async fn call_method(
        &self,
        method: &str,
        args: Option<HashMap<String, String>>,
    ) -> Result<String, JssError> {
        let path = format!("/api/method/{method}");
        let body = if let Some(args) = args {
            serde_json::to_string(&args)
                .map_err(|e| JssError::Parse(format!("Failed to serialize method args: {e}")))?
        } else {
            "{}".to_string()
        };
        self.authenticated_post(&path, &body).await
    }
}
