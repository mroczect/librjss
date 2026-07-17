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

    #[serde(default)]
    pub user_info: HashMap<String, BootUserInfo>,
    #[serde(default)]
    pub sidebar_pages: SidebarPages,
    #[serde(default)]
    pub navbar_settings: Option<NavbarSettings>,
    #[serde(default)]
    pub versions: HashMap<String, String>,
    #[serde(default)]
    pub lang_dict: HashMap<String, String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub timezone_info: Option<serde_json::Value>,
    #[serde(default)]
    pub page_info: HashMap<String, PageInfo>,
    #[serde(default)]
    pub frequently_visited_links: Vec<FrequentLink>,
    #[serde(default)]
    pub developer_mode: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub socketio_port: Option<u16>,
    #[serde(default)]
    pub desk_settings: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub desk_theme: Option<String>,
    #[serde(default)]
    pub versions: HashMap<String, String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub timezone_info: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BootUserInfo {
    #[serde(default)]
    pub fullname: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SidebarPages {
    #[serde(default)]
    pub workspace_setup_completed: Option<i32>,
    #[serde(default)]
    pub pages: Vec<SidebarPage>,
    #[serde(default)]
    pub has_access: bool,
    #[serde(default)]
    pub has_create_access: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SidebarPage {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub for_user: Option<String>,
    #[serde(default)]
    pub parent_page: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub public: Option<i32>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub indicator_color: Option<String>,
    #[serde(default)]
    pub is_hidden: Option<i32>,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub link_type: Option<String>,
    #[serde(default)]
    pub link_to: Option<String>,
    #[serde(default)]
    pub external_link: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NavbarSettings {
    #[serde(default)]
    pub settings_dropdown: Vec<NavbarItem>,
    #[serde(default)]
    pub help_dropdown: Vec<NavbarItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NavbarItem {
    #[serde(default)]
    pub item_label: String,
    #[serde(default)]
    pub item_type: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub hidden: Option<i32>,
    #[serde(default)]
    pub is_standard: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PageInfo {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FrequentLink {
    pub route: String,
    pub count: i32,
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
