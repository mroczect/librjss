use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct FrappeBoot {
    #[serde(default)]
    pub user: BootUser,
    #[serde(default)]
    pub sitename: String,
    #[serde(default)]
    pub csrf_token: String,
    #[serde(default)]
    pub sysdefaults: SysDefaults,
    #[serde(default)]
    pub app_logo_url: Option<String>,
    #[serde(default)]
    pub home_page: Option<String>,

    #[serde(default)]
    pub allow_modules: Vec<String>,
    #[serde(default)]
    pub can_select: Vec<String>,
    #[serde(default)]
    pub can_create: Vec<String>,
    #[serde(default)]
    pub can_write: Vec<String>,
    #[serde(default)]
    pub can_read: Vec<String>,
    #[serde(default)]
    pub can_submit: Vec<String>,
    #[serde(default)]
    pub can_cancel: Vec<String>,
    #[serde(default)]
    pub can_delete: Vec<String>,
    #[serde(default)]
    pub can_get_report: Vec<String>,
    #[serde(default)]
    pub all_reports: HashMap<String, ReportMeta>,
    #[serde(default)]
    pub module_wise_workspaces: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub dashboards: Vec<DashboardMeta>,
    #[serde(default)]
    pub single_types: Vec<String>,
    #[serde(default)]
    pub calendars: Vec<String>,
    #[serde(default)]
    pub treeviews: Vec<String>,
    #[serde(default)]
    pub module_app: HashMap<String, String>,
    #[serde(default)]
    pub app_data: Vec<AppData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportMeta {
    #[serde(default)]
    pub modified: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub ref_doctype: String,
    #[serde(default)]
    pub report_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardMeta {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppData {
    pub app_name: String,
    pub app_title: String,
    pub app_route: String,
    #[serde(default)]
    pub app_logo_url: serde_json::Value,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub workspaces: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BootUser {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub allow_modules: Vec<String>,
    #[serde(default)]
    pub user_type: String,
    #[serde(default)]
    pub permissions: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub can_select: Vec<String>,
    #[serde(default)]
    pub can_create: Vec<String>,
    #[serde(default)]
    pub can_write: Vec<String>,
    #[serde(default)]
    pub can_read: Vec<String>,
    #[serde(default)]
    pub can_submit: Vec<String>,
    #[serde(default)]
    pub can_cancel: Vec<String>,
    #[serde(default)]
    pub can_delete: Vec<String>,
    #[serde(default)]
    pub can_get_report: Vec<String>,
    #[serde(default)]
    pub all_reports: HashMap<String, ReportMeta>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SysDefaults {
    #[serde(default)]
    pub default_app: Option<String>,
    #[serde(default)]
    pub time_zone: Option<String>,
}
