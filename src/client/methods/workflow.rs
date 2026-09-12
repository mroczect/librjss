use crate::client::RjssClient;
use crate::handler::error::JssError;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowTransition {
    pub action: String,
    pub state: String,
    #[serde(default)]
    pub next_state: Option<String>,
    #[serde(default)]
    pub allowed: Option<String>,
    #[serde(default)]
    pub allow_self_approval: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocRef<'a> {
    pub doctype: &'a str,
    pub name: &'a str,
}

impl RjssClient {
    #[instrument(skip(self))]
    pub async fn workflow_transitions(
        &self,
        doctype: &str,
        name: &str,
    ) -> Result<Vec<WorkflowTransition>, JssError> {
        let doc_json = serde_json::to_string(&DocRef { doctype, name })
            .map_err(|e| JssError::Parse(e.to_string()))?;
        let form = [("doc", doc_json.as_str())];
        let raw = self
            .post_form("/api/method/frappe.model.workflow.get_transitions", &form)
            .await?;
        #[derive(Deserialize)]
        struct Envelope {
            message: Vec<WorkflowTransition>,
        }
        let env: Envelope = serde_json::from_str(&raw)
            .map_err(|e| JssError::Parse(format!("workflow_transitions parse: {e}")))?;
        Ok(env.message)
    }

    #[instrument(skip(self))]
    pub async fn apply_workflow(
        &self,
        doctype: &str,
        name: &str,
        action: &str,
    ) -> Result<serde_json::Value, JssError> {
        #[derive(Serialize)]
        struct ApplyBody<'a> {
            doc: DocRef<'a>,
            action: &'a str,
        }
        let body = ApplyBody {
            doc: DocRef { doctype, name },
            action,
        };
        let body_str = serde_json::to_string(&body).map_err(|e| JssError::Parse(e.to_string()))?;
        let raw = self
            .authenticated_post(
                "/api/method/frappe.model.workflow.apply_workflow",
                &body_str,
            )
            .await?;
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(e.to_string()))
    }

    #[instrument(skip(self))]
    pub async fn transition_to_state(
        &self,
        doctype: &str,
        name: &str,
        target_state: &str,
    ) -> Result<serde_json::Value, JssError> {
        let transitions = self.workflow_transitions(doctype, name).await?;
        let t = transitions
            .into_iter()
            .find(|t| t.next_state.as_deref() == Some(target_state))
            .ok_or_else(|| {
                JssError::Validation(format!(
                    "Tidak ada transisi ke state '{target_state}' untuk {doctype}/{name}"
                ))
            })?;
        self.apply_workflow(doctype, name, &t.action).await
    }
}
