use reqwest::Url;

pub trait AuthEndpoints {
    fn login_url(base: &Url) -> Url {
        base.join("/api/method/login").unwrap()
    }
    fn logout_url(base: &Url) -> Url {
        base.join("/api/method/logout").unwrap()
    }
    fn csrf_token_url(base: &Url) -> Url {
        base.join("/api/method/frappe.auth.get_csrf_token").unwrap()
    }
    fn get_logged_user_url(base: &Url) -> Url {
        base.join("/api/method/frappe.auth.get_logged_user")
            .unwrap()
    }
    fn app_page_url(base: &Url) -> Url {
        base.join("/app").unwrap()
    }
}
