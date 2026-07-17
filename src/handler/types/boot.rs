use serde::Deserialize;
use std::collections::HashMap;

/// Represents the `frappe.boot` object from the Frappe dashboard page.
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
}

/// User information embedded in `frappe.boot`.
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
}

/// System defaults embedded in `frappe.boot`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SysDefaults {
    #[serde(default)]
    pub default_app: Option<String>,
    #[serde(default)]
    pub time_zone: Option<String>,
}
