use crate::handler::error::JssError;

pub struct ReadOnlyGuard {
    safe_post_paths: Vec<&'static str>,
}

impl ReadOnlyGuard {
    pub fn new() -> Self {
        Self {
            safe_post_paths: vec![
                "/api/method/login",
                "/api/method/logout",
                "/api/method/frappe.sessions.get_csrf_token",
                "/api/method/frappe.auth.get_logged_user",
                "/api/method/frappe.client.get_count",
                "/api/method/frappe.client.get_list",
                "/api/method/frappe.client.get_value",
                "/api/method/frappe.client.get",
                "/api/method/frappe.client.validate_link",
                "/api/method/frappe.desk.reportview.get_list",
                "/api/method/frappe.desk.reportview.get_count",
                "/api/method/frappe.desk.search.search_link",
                "/api/method/frappe.desk.form.load.getdoc",
                "/api/method/frappe.desk.form.load.getdoctype",
                "/api/method/frappe.desk.desktop.get_desktop_page",
                "/api/method/frappe.utils.global_search.search",
                "/api/method/frappe.model.workflow.get_transitions",
                "/api/method/juragan.collection.utils.get_pic_belum_lapor_hari_ini",
                "/api/method/juragan.collection.utils.get_dokumen_belum_dikonfirmasi",
                "/api/method/juragan.collection.utils.get_dokumen_overdue_collection",
                "/api/method/juragan.collection.utils.get_combined_collection_data",
                "/api/method/juragan.ops.doctype.master_data_nasabah.master_data_nasabah.cek_rincian_hutang_nasabah",
            ],
        }
    }

    pub fn check(&self, method: &str, path: &str) -> Result<(), JssError> {
        let m = method.to_ascii_uppercase();
        if m == "GET" || m == "HEAD" || m == "OPTIONS" {
            return Ok(());
        }
        for safe in &self.safe_post_paths {
            if path.starts_with(safe) {
                return Ok(());
            }
        }
        Err(JssError::Validation(format!(
            "🛡️  READ-ONLY GUARD menolak {} {}\n\
             Alasan: path tidak ada di whitelist read-only.\n\
             Kalau kamu memang mau mutasi, set `readonly_guard: false` di ClientConfig.",
            m, path
        )))
    }
}

impl Default for ReadOnlyGuard {
    fn default() -> Self {
        Self::new()
    }
}
