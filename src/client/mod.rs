pub mod auth;
pub mod file;
pub mod methods;
pub mod report;
pub mod resource;

use reqwest::Client as ReqwestClient;
use reqwest::Url;
use reqwest::cookie::Jar;
use secrecy::ExposeSecret;
use secrecy::SecretString;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::api::auth::AuthEndpoints;
use crate::handler::config::ClientConfig;
use crate::handler::error::JssError;
use crate::handler::types::SessionInfo;
use crate::handler::types::boot::FrappeBoot;

pub struct RjssClient {
    pub(crate) config: ClientConfig,
    pub(crate) http: ReqwestClient,
    pub(crate) session: Option<SessionInfo>,
    pub(crate) trace_id: String,
    pub(crate) credentials: Option<(SecretString, SecretString)>,
    pub(crate) boot: Option<FrappeBoot>,
}

impl AuthEndpoints for RjssClient {}

impl RjssClient {
    pub fn new(config: ClientConfig) -> Result<Self, JssError> {
        config.validate()?;

        let cookie_jar = Arc::new(Jar::default());
        let http = ReqwestClient::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .danger_accept_invalid_certs(config.insecure_ssl)
            .user_agent(&config.user_agent)
            .build()?;

        let trace_id = Uuid::new_v4().to_string();
        info!(
            trace_id,
            "Client created with base_url: {}", config.base_url
        );

        Ok(RjssClient {
            config,
            http,
            session: None,
            trace_id,
            credentials: None,
            boot: None,
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.config.base_url
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn session_info(&self) -> Option<&SessionInfo> {
        self.session.as_ref()
    }

    pub fn boot(&self) -> Option<&FrappeBoot> {
        self.boot.as_ref()
    }

    pub async fn authenticate(&mut self) -> Result<(), JssError> {
        match self.config.auth_mode.clone() {
            crate::handler::config::AuthMode::Session { email, password } => {
                self.credentials = Some((email.clone(), password.clone()));
                auth::login::login_with_credentials(self, email, password).await
            }
            crate::handler::config::AuthMode::Token { .. } => {
                let session =
                    auth::token::setup_token_session(self.config.expected_sitename.clone());
                self.session = Some(session);
                Ok(())
            }
        }
    }

    pub async fn logout(&mut self) -> Result<(), JssError> {
        auth::logout::logout(self).await
    }

    pub async fn ensure_session(&mut self) -> Result<(), JssError> {
        auth::session::ensure_session(self).await
    }

    pub async fn authenticated_get(&self, path: &str) -> Result<String, JssError> {
        methods::get::authenticated_get(self, path).await
    }

    pub async fn authenticated_post(
        &self,
        path: &str,
        body_json: &str,
    ) -> Result<String, JssError> {
        methods::post::authenticated_post(self, path, body_json).await
    }

    pub async fn authenticated_put(&self, path: &str, body_json: &str) -> Result<String, JssError> {
        methods::put::authenticated_put(self, path, body_json).await
    }

    pub async fn authenticated_delete(&self, path: &str) -> Result<String, JssError> {
        methods::delete::authenticated_delete(self, path).await
    }

    pub fn user_info_map(
        &self,
    ) -> Option<&std::collections::HashMap<String, crate::handler::types::BootUserInfo>> {
        self.boot.as_ref().map(|b| &b.user_info)
    }

    pub fn sidebar_pages(&self) -> Option<&crate::handler::types::SidebarPages> {
        self.boot.as_ref().map(|b| &b.sidebar_pages)
    }

    pub fn navbar_settings(&self) -> Option<&crate::handler::types::NavbarSettings> {
        self.boot.as_ref().and_then(|b| b.navbar_settings.as_ref())
    }

    pub fn versions(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.boot.as_ref().map(|b| &b.versions)
    }

    pub fn lang_dict(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.boot.as_ref().map(|b| &b.lang_dict)
    }

    pub fn frequent_links(&self) -> Option<&[crate::handler::types::FrequentLink]> {
        self.boot
            .as_ref()
            .map(|b| b.frequently_visited_links.as_slice())
    }

    pub fn is_developer_mode(&self) -> bool {
        self.boot
            .as_ref()
            .map(|b| b.developer_mode != 0)
            .unwrap_or(false)
    }

    pub fn is_read_only(&self) -> bool {
        self.boot.as_ref().map(|b| b.read_only).unwrap_or(false)
    }
    pub async fn post_form(&self, path: &str, form: &[(&str, &str)]) -> Result<String, JssError> {
        crate::client::methods::post_form::authenticated_post_form(self, path, form).await
    }

    pub async fn global_search(
        &self,
        query: &str,
        limit: u32,
        doctype: Option<&str>,
    ) -> Result<String, JssError> {
        let form = [
            ("text", query),
            ("start", "0"),
            ("limit", &limit.to_string()),
            ("doctype", doctype.unwrap_or("")),
        ];
        self.post_form("/api/method/frappe.utils.global_search.search", &form)
            .await
    }

    pub async fn search_link(
        &self,
        txt: &str,
        doctype: &str,
        reference_doctype: &str,
        page_length: u32,
    ) -> Result<String, JssError> {
        let form = [
            ("txt", txt),
            ("doctype", doctype),
            ("reference_doctype", reference_doctype),
            ("page_length", &page_length.to_string()),
        ];
        self.post_form("/api/method/frappe.desk.search.search_link", &form)
            .await
    }

    pub async fn get_transitions(&self, doc_json: &str) -> Result<String, JssError> {
        let form = [("doc", doc_json)];
        self.post_form("/api/method/frappe.model.workflow.get_transitions", &form)
            .await
    }

    pub async fn save_user_settings(
        &self,
        doctype: &str,
        user_settings: &str,
    ) -> Result<String, JssError> {
        let form = [("doctype", doctype), ("user_settings", user_settings)];
        self.post_form("/api/method/frappe.model.utils.user_settings.save", &form)
            .await
    }

    pub async fn get_count(
        &self,
        doctype: &str,
        filters_json: &str,
        fields_json: &str,
        distinct: bool,
    ) -> Result<String, JssError> {
        let form = [
            ("doctype", doctype),
            ("filters", filters_json),
            ("fields", fields_json),
            ("distinct", &distinct.to_string()),
        ];
        self.post_form("/api/method/frappe.desk.reportview.get_count", &form)
            .await
    }

    pub async fn get_list(
        &self,
        doctype: &str,
        fields_json: &str,
        filters_json: &str,
        or_filters_json: &str,
        order_by: &str,
        start: u32,
        page_length: u32,
        view: &str,
        group_by: &str,
        with_comment_count: bool,
    ) -> Result<String, JssError> {
        let params = vec![
            format!("doctype={}", urlencoding::encode(doctype)),
            format!("fields={}", urlencoding::encode(fields_json)),
            format!("filters={}", urlencoding::encode(filters_json)),
            format!("or_filters={}", urlencoding::encode(or_filters_json)),
            format!("order_by={}", urlencoding::encode(order_by)),
            format!("start={start}"),
            format!("page_length={page_length}"),
            format!("view={}", urlencoding::encode(view)),
            format!("group_by={}", urlencoding::encode(group_by)),
            format!("with_comment_count={}", with_comment_count as i32),
        ];
        let path = format!(
            "/api/method/frappe.desk.reportview.get_list?{}",
            params.join("&")
        );
        self.authenticated_get(&path).await
    }

    pub async fn get_list_settings(&self, doctype: &str) -> Result<String, JssError> {
        let form = [("doctype", doctype)];
        self.post_form("/api/method/frappe.desk.listview.get_list_settings", &form)
            .await
    }

    pub async fn get_doctype_meta(&self, doctype: &str) -> Result<String, JssError> {
        let path = format!(
            "/api/method/frappe.desk.form.load.getdoctype?doctype={}&with_parent=1",
            urlencoding::encode(doctype)
        );
        self.authenticated_get(&path).await
    }

    pub async fn file_list(
        &self,
        folder: &str,
        limit: u32,
        extra_filters: Option<Vec<(&str, &str, &str, &str)>>,
    ) -> Result<String, JssError> {
        let mut all_filters: Vec<(&str, &str, &str, &str)> = vec![("File", "folder", "=", folder)];

        if let Some(ef) = extra_filters {
            for f in ef {
                all_filters.push(f);
            }
        }

        let filters_formatted: Vec<[&str; 4]> = all_filters
            .iter()
            .map(|(d, f, o, v)| [*d, *f, *o, *v])
            .collect();

        let filters_json = serde_json::to_string(&filters_formatted).unwrap_or_default();

        let fields = [
            "file_name",
            "file_url",
            "file_size",
            "file_type",
            "is_private",
            "attached_to_doctype",
            "attached_to_name",
            "attached_to_field",
            "creation",
            "modified",
        ];
        let fields_json = serde_json::to_string(&fields).unwrap_or_default();

        let path = format!(
            "/api/resource/File?filters={}&fields={}&limit_page_length={limit}&order_by=creation%20desc",
            urlencoding::encode(&filters_json),
            urlencoding::encode(&fields_json)
        );

        self.authenticated_get(&path).await
    }

    pub async fn get_doc_json(
        &self,
        doctype: &str,
        name: &str,
    ) -> Result<serde_json::Value, JssError> {
        let body = self.get_doc(doctype, name).await?;
        serde_json::from_str(&body).map_err(|e| JssError::Parse(e.to_string()))
    }

    pub async fn doctype_list_json(
        &self,
        doctype: &str,
        fields: Option<Vec<&str>>,
        filters: Vec<(&str, &str, &str)>,
        limit: u32,
        start: u32,
        order_by: Option<&str>,
    ) -> Result<serde_json::Value, JssError> {
        let mut builder = self.doctype(doctype).limit(limit).limit_start(start);
        if let Some(f) = fields {
            builder = builder.fields(f);
        }
        if let Some(o) = order_by {
            builder = builder.order_by(o);
        }
        for (field, op, value) in filters {
            builder = builder.filter(field, op, value);
        }
        let raw = builder.execute_raw().await?;
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(e.to_string()))
    }

    pub async fn run_report(
        &self,
        report_name: &str,
        filters: serde_json::Value,
    ) -> Result<String, JssError> {
        let body = serde_json::json!({ "report_name": report_name, "filters": filters });
        self.authenticated_post(
            "/api/method/frappe.desk.query_report.run",
            &body.to_string(),
        )
        .await
    }

    pub async fn get_notifications(&self, limit: u32) -> Result<String, JssError> {
        let path = format!(
            "/api/method/frappe.desk.doctype.notification_log.notification_log.get_notification_logs?limit={limit}"
        );
        self.authenticated_get(&path).await
    }

    pub async fn get_events(&self, start: &str, end: &str) -> Result<String, JssError> {
        let path = format!(
            "/api/method/frappe.desk.doctype.event.event.get_events?start={start}&end={end}"
        );
        self.authenticated_get(&path).await
    }

    pub async fn get_desktop_page(
        &self,
        name: &str,
        title: &str,
        public: bool,
    ) -> Result<String, JssError> {
        let page = serde_json::json!({ "name": name, "title": title, "public": public });
        let path = format!(
            "/api/method/frappe.desk.desktop.get_desktop_page?page={}",
            urlencoding::encode(&page.to_string())
        );
        self.authenticated_get(&path).await
    }

    pub async fn get_lazy_child_rows(&self, docname: &str, tab: &str) -> Result<String, JssError> {
        let form = [("docname", docname), ("tab", tab)];
        self.post_form("/api/method/juragan.ops.doctype.master_data_nasabah.master_data_nasabah.get_lazy_child_rows", &form).await
    }

    pub async fn download_pdf_kartu_piutang(
        &self,
        doctype: &str,
        name: &str,
        format: &str,
        no_letterhead: bool,
    ) -> Result<Vec<u8>, JssError> {
        let no_letterhead_val = if no_letterhead { "1" } else { "0" };

        let path = format!(
            "/api/method/frappe.utils.print_format.download_pdf?doctype={}&name={}&format={}&no_letterhead={}",
            urlencoding::encode(doctype),
            urlencoding::encode(name),
            urlencoding::encode(format),
            no_letterhead_val
        );

        let url = self
            .config
            .base_url
            .join(&path)
            .map_err(|e| JssError::Parse(format!("Invalid PDF URL: {e}")))?;

        let mut req = self.http.get(url);

        if let Some(session) = &self.session {
            let csrf = session.csrf_token.expose_secret();
            if !csrf.is_empty() {
                req = req.header("X-Frappe-CSRF-Token", csrf);
            }
        }

        req = crate::client::auth::http_helpers::apply_auth_to_builder(&self.config.auth_mode, req);

        let resp = req.send().await?;
        let status = resp.status();

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(JssError::from_api_response(status, &body));
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}
