use crate::client::RjssClient;
use crate::handler::error::JssError;
use std::path::Path;
use tracing::instrument;

impl RjssClient {
    #[instrument(skip(self, file_content))]
    pub async fn upload_file(
        &self,
        file_name: &str,
        file_content: Vec<u8>,
        doctype: &str,
        docname: &str,
        fieldname: &str,
    ) -> Result<String, JssError> {
        let url = self
            .base_url()
            .join("/api/method/upload_file")
            .map_err(|e| JssError::Parse(format!("Invalid URL: {e}")))?;

        let part = reqwest::multipart::Part::bytes(file_content).file_name(file_name.to_string());

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("doctype", doctype.to_string())
            .text("docname", docname.to_string())
            .text("fieldname", fieldname.to_string());

        let response = self.http.post(url).multipart(form).send().await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(JssError::from_api_response(status, &body));
        }

        Ok(body)
    }

    #[instrument(skip(self))]
    pub async fn download_file(&self, file_url: &str) -> Result<Vec<u8>, JssError> {
        let url = self
            .base_url()
            .join(file_url)
            .map_err(|e| JssError::Parse(format!("Invalid file URL: {e}")))?;

        let response = self.http.get(url).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(JssError::from_api_response(status, &body));
        }

        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    #[instrument(skip(self))]
    pub async fn download_file_to_path(
        &self,
        file_url: &str,
        save_path: &Path,
    ) -> Result<(), JssError> {
        let bytes = self.download_file(file_url).await?;
        std::fs::write(save_path, bytes)
            .map_err(|e| JssError::FileOperation(format!("Failed to save file: {e}")))?;
        Ok(())
    }
}
