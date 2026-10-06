#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::OnceLock;

use librjss::{AuthMode, ClientConfig, JssError, RjssClient};
use secrecy::SecretString;
use tokio::runtime::Runtime;

// ───────────────────────── error codes ─────────────────────────

pub const JSS_OK: i32 = 0;
pub const JSS_ERR_CONFIG: i32 = -1;
pub const JSS_ERR_VALIDATION: i32 = -2;
pub const JSS_ERR_NETWORK: i32 = -3;
pub const JSS_ERR_HTTP: i32 = -4;
pub const JSS_ERR_API: i32 = -5;
pub const JSS_ERR_AUTH: i32 = -6;
pub const JSS_ERR_CSRF: i32 = -7;
pub const JSS_ERR_PERMISSION: i32 = -8;
pub const JSS_ERR_SITENAME_MISMATCH: i32 = -9;
pub const JSS_ERR_NOT_AUTHENTICATED: i32 = -10;
pub const JSS_ERR_RATE_LIMITED: i32 = -11;
pub const JSS_ERR_PARSE: i32 = -12;
pub const JSS_ERR_EXPIRED: i32 = -13;
pub const JSS_ERR_CANCELLED: i32 = -14;
pub const JSS_ERR_FILE_OPERATION: i32 = -15;
pub const JSS_ERR_INTERNAL: i32 = -16;
pub const JSS_ERR_NULL_POINTER: i32 = -17;
pub const JSS_ERR_UTF8: i32 = -18;
pub const JSS_ERR_PANIC: i32 = -19;

pub const JSS_FLAG_INSECURE_SSL: u32 = 1 << 0;
pub const JSS_FLAG_NO_READONLY_GUARD: u32 = 1 << 1;

fn error_code(e: &JssError) -> i32 {
    match e {
        JssError::Config(_) => JSS_ERR_CONFIG,
        JssError::Validation(_) => JSS_ERR_VALIDATION,
        JssError::Network(_) => JSS_ERR_NETWORK,
        JssError::Http { .. } => JSS_ERR_HTTP,
        JssError::ApiError { .. } => JSS_ERR_API,
        JssError::Auth(_) => JSS_ERR_AUTH,
        JssError::Csrf(_) => JSS_ERR_CSRF,
        JssError::Permission(_) => JSS_ERR_PERMISSION,
        JssError::SitenameMismatch { .. } => JSS_ERR_SITENAME_MISMATCH,
        JssError::NotAuthenticated => JSS_ERR_NOT_AUTHENTICATED,
        JssError::RateLimited { .. } => JSS_ERR_RATE_LIMITED,
        JssError::Parse(_) => JSS_ERR_PARSE,
        JssError::Expired => JSS_ERR_EXPIRED,
        JssError::Cancelled => JSS_ERR_CANCELLED,
        JssError::FileOperation(_) => JSS_ERR_FILE_OPERATION,
        JssError::Internal(_) => JSS_ERR_INTERNAL,
    }
}

// ───────────────────────── last-error slot ─────────────────────

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    let s = msg.into().replace('\0', "\\0");
    let c = CString::new(s).unwrap_or_default();
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(c));
}

#[unsafe(no_mangle)]
pub extern "C" fn jss_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| match &*slot.borrow() {
        Some(c) => c.as_ptr(),
        None => ptr::null(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn jss_version() -> *const c_char {
    c"librjss-ffi 2.3.0".as_ptr()
}

// ───────────────────────── runtime & helpers ───────────────────

fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    runtime().block_on(f)
}

pub struct JssClient {
    inner: RjssClient,
}

#[repr(C)]
pub struct JssClientConfig {
    pub base_url: *const c_char,
    pub auth_kind: *const c_char,
    pub principal: *const c_char,
    pub secret: *const c_char,
    pub expected_sitename: *const c_char,
    pub flags: u32,
    pub _reserved: u32,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub _reserved2: u32,
}

unsafe fn cstr<'a>(p: *const c_char, name: &str) -> Result<&'a str, JssError> {
    if p.is_null() {
        return Err(JssError::Validation(format!("{name} is NULL")));
    }
    CStr::from_ptr(p)
        .to_str()
        .map_err(|e| JssError::Validation(format!("{name} is not valid UTF-8: {e}")))
}

unsafe fn opt_cstr<'a>(p: *const c_char, name: &str) -> Result<Option<&'a str>, JssError> {
    if p.is_null() {
        return Ok(None);
    }
    CStr::from_ptr(p)
        .to_str()
        .map(Some)
        .map_err(|e| JssError::Validation(format!("{name} is not valid UTF-8: {e}")))
}

unsafe fn client_ref<'a>(p: *mut JssClient) -> Result<&'a JssClient, JssError> {
    if p.is_null() {
        return Err(JssError::Validation("JssClient* is NULL".into()));
    }
    Ok(&*p)
}

unsafe fn client_mut<'a>(p: *mut JssClient) -> Result<&'a mut JssClient, JssError> {
    if p.is_null() {
        return Err(JssError::Validation("JssClient* is NULL".into()));
    }
    Ok(&mut *p)
}

fn write_out_string(out: *mut *mut c_char, s: String) -> Result<(), JssError> {
    if out.is_null() {
        return Ok(());
    }
    let sanitized = s.replace('\0', "\\0");
    let c = CString::new(sanitized).map_err(|e| JssError::Internal(format!("CString: {e}")))?;
    unsafe { *out = c.into_raw() };
    Ok(())
}

fn write_out_bytes(
    out_data: *mut *mut u8,
    out_len: *mut usize,
    v: Vec<u8>,
) -> Result<(), JssError> {
    if out_data.is_null() || out_len.is_null() {
        return Err(JssError::Validation("out_data/out_len is NULL".into()));
    }
    let mut boxed = v.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    unsafe {
        *out_data = ptr;
        *out_len = len;
    }
    Ok(())
}

fn clear_string_out(out: *mut *mut c_char) {
    if !out.is_null() {
        unsafe { *out = ptr::null_mut() };
    }
}

fn clear_bytes_out(out_data: *mut *mut u8, out_len: *mut usize) {
    if !out_data.is_null() {
        unsafe { *out_data = ptr::null_mut() };
    }
    if !out_len.is_null() {
        unsafe { *out_len = 0 };
    }
}

fn handle_panic() -> i32 {
    set_last_error("panic in librjss-ffi (see stderr)");
    JSS_ERR_PANIC
}

// ───────────────────────── internal dispatchers ────────────────

fn run_string_op<F>(out: *mut *mut c_char, f: F) -> i32
where
    F: FnOnce() -> Result<String, JssError>,
{
    clear_string_out(out);
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(s)) => match write_out_string(out, s) {
            Ok(_) => JSS_OK,
            Err(e) => {
                set_last_error(e.to_string());
                error_code(&e)
            }
        },
        Ok(Err(e)) => {
            set_last_error(e.to_string());
            error_code(&e)
        }
        Err(_) => handle_panic(),
    }
}

fn run_bytes_op<F>(out_data: *mut *mut u8, out_len: *mut usize, f: F) -> i32
where
    F: FnOnce() -> Result<Vec<u8>, JssError>,
{
    clear_bytes_out(out_data, out_len);
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(b)) => match write_out_bytes(out_data, out_len, b) {
            Ok(_) => JSS_OK,
            Err(e) => {
                set_last_error(e.to_string());
                error_code(&e)
            }
        },
        Ok(Err(e)) => {
            set_last_error(e.to_string());
            error_code(&e)
        }
        Err(_) => handle_panic(),
    }
}

fn run_bool_op<F>(f: F) -> i32
where
    F: FnOnce() -> Result<bool, JssError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(true)) => 1,
        Ok(Ok(false)) => 0,
        Ok(Err(e)) => {
            set_last_error(e.to_string());
            error_code(&e)
        }
        Err(_) => handle_panic(),
    }
}

fn run_unit_op<F>(f: F) -> i32
where
    F: FnOnce() -> Result<(), JssError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => JSS_OK,
        Ok(Err(e)) => {
            set_last_error(e.to_string());
            error_code(&e)
        }
        Err(_) => handle_panic(),
    }
}

// ───────────────────────── lifecycle ───────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_new(cfg: *const JssClientConfig) -> *mut JssClient {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<*mut JssClient, JssError> {
        if cfg.is_null() {
            return Err(JssError::Validation("JssClientConfig* is NULL".into()));
        }
        let cfg = &*cfg;

        let base_url = cstr(cfg.base_url, "base_url")?
            .parse::<reqwest::Url>()
            .map_err(|e| JssError::Config(format!("base_url invalid: {e}")))?;
        let auth_kind = cstr(cfg.auth_kind, "auth_kind")?;
        let principal = cstr(cfg.principal, "principal")?;
        let secret = cstr(cfg.secret, "secret")?;
        let expected_sitename =
            opt_cstr(cfg.expected_sitename, "expected_sitename")?.map(|s| s.to_string());

        let auth_mode = match auth_kind {
            "session" => AuthMode::Session {
                email: SecretString::new(principal.to_string().into_boxed_str()),
                password: SecretString::new(secret.to_string().into_boxed_str()),
            },
            "token" => AuthMode::Token {
                api_key: principal.to_string(),
                api_secret: SecretString::new(secret.to_string().into_boxed_str()),
            },
            other => {
                return Err(JssError::Config(format!(
                    "auth_kind must be 'session' or 'token', got '{other}'"
                )));
            }
        };

        let config = ClientConfig {
            base_url,
            auth_mode,
            expected_sitename,
            required_roles: Vec::new(),
            timeout_secs: if cfg.timeout_secs == 0 {
                30
            } else {
                cfg.timeout_secs
            },
            max_retries: if cfg.max_retries == 0 {
                3
            } else {
                cfg.max_retries
            },
            user_agent: "librjss-ffi/2.3.0".into(),
            insecure_ssl: cfg.flags & JSS_FLAG_INSECURE_SSL != 0,
            readonly_guard: cfg.flags & JSS_FLAG_NO_READONLY_GUARD == 0,
        };

        let inner = RjssClient::new(config)?;
        Ok(Box::into_raw(Box::new(JssClient { inner })))
    }));

    match result {
        Ok(Ok(p)) => {
            set_last_error("");
            p
        }
        Ok(Err(e)) => {
            set_last_error(e.to_string());
            ptr::null_mut()
        }
        Err(_) => {
            handle_panic();
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_free(c: *mut JssClient) {
    if c.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(c));
    }));
}

// ───────────────────────── simple info ─────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_trace_id(c: *mut JssClient, out: *mut *mut c_char) -> i32 {
    run_string_op(out, || {
        let c = client_ref(c)?;
        Ok(c.inner.trace_id().to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_is_authenticated(c: *mut JssClient) -> i32 {
    run_bool_op(|| {
        let c = client_ref(c)?;
        Ok(c.inner.session_info().is_some())
    })
}

// ───────────────────────── auth lifecycle ──────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_authenticate(c: *mut JssClient) -> i32 {
    run_unit_op(|| {
        let c = client_mut(c)?;
        block_on(c.inner.authenticate())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_logout(c: *mut JssClient) -> i32 {
    run_unit_op(|| {
        let c = client_mut(c)?;
        block_on(c.inner.logout())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_ensure_session(c: *mut JssClient) -> i32 {
    run_unit_op(|| {
        let c = client_mut(c)?;
        block_on(c.inner.ensure_session())
    })
}

// ───────────────────────── HTTP verbs ──────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_get(
    c: *mut JssClient,
    path: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(client.inner.authenticated_get(cstr(path, "path")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_delete(
    c: *mut JssClient,
    path: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(client.inner.authenticated_delete(cstr(path, "path")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_post(
    c: *mut JssClient,
    path: *const c_char,
    body_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(
            client
                .inner
                .authenticated_post(cstr(path, "path")?, cstr(body_json, "body_json")?),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_put(
    c: *mut JssClient,
    path: *const c_char,
    body_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(
            client
                .inner
                .authenticated_put(cstr(path, "path")?, cstr(body_json, "body_json")?),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_post_form(
    c: *mut JssClient,
    path: *const c_char,
    keys: *const *const c_char,
    values: *const *const c_char,
    n_pairs: usize,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let path = cstr(path, "path")?;

        if n_pairs > 0 && (keys.is_null() || values.is_null()) {
            return Err(JssError::Validation("keys/values is NULL".into()));
        }

        let mut pairs: Vec<(&str, &str)> = Vec::with_capacity(n_pairs);
        for i in 0..n_pairs {
            let k = cstr(*keys.add(i), "keys[i]")?;
            let v = cstr(*values.add(i), "values[i]")?;
            pairs.push((k, v));
        }
        block_on(client.inner.post_form(path, &pairs))
    })
}

// ───────────────────────── method calls ────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_call_method(
    c: *mut JssClient,
    method: *const c_char,
    args_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let method = cstr(method, "method")?;

        let mut map = std::collections::HashMap::new();
        if !args_json.is_null() {
            let raw = cstr(args_json, "args_json")?;
            if !raw.trim().is_empty() {
                let v: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(raw)
                        .map_err(|e| JssError::Parse(format!("args_json: {e}")))?;
                for (k, val) in v {
                    let s = match val {
                        serde_json::Value::String(s) => s,
                        other => other.to_string(),
                    };
                    map.insert(k, s);
                }
            }
        }

        let args = if map.is_empty() { None } else { Some(map) };
        block_on(client.inner.call_method(method, args))
    })
}

// ───────────────────────── resource CRUD ───────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_get_doc(
    c: *mut JssClient,
    doctype: *const c_char,
    name: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(
            client
                .inner
                .get_doc(cstr(doctype, "doctype")?, cstr(name, "name")?),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_create_doc(
    c: *mut JssClient,
    doctype: *const c_char,
    data_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let dt = cstr(doctype, "doctype")?;
        let v: serde_json::Value = serde_json::from_str(cstr(data_json, "data_json")?)
            .map_err(|e| JssError::Parse(format!("data_json: {e}")))?;
        block_on(client.inner.create_doc(dt, &v))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_update_doc(
    c: *mut JssClient,
    doctype: *const c_char,
    name: *const c_char,
    data_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let dt = cstr(doctype, "doctype")?;
        let n = cstr(name, "name")?;
        let v: serde_json::Value = serde_json::from_str(cstr(data_json, "data_json")?)
            .map_err(|e| JssError::Parse(format!("data_json: {e}")))?;
        block_on(client.inner.update_doc(dt, n, &v))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_delete_doc(
    c: *mut JssClient,
    doctype: *const c_char,
    name: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        block_on(
            client
                .inner
                .delete_doc(cstr(doctype, "doctype")?, cstr(name, "name")?),
        )
    })
}

// ───────────────────────── files ───────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_upload_file(
    c: *mut JssClient,
    file_name: *const c_char,
    content: *const u8,
    content_len: usize,
    doctype: *const c_char,
    docname: *const c_char,
    fieldname: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        if content.is_null() && content_len > 0 {
            return Err(JssError::Validation("content is NULL".into()));
        }
        let bytes = std::slice::from_raw_parts(content, content_len).to_vec();
        block_on(client.inner.upload_file(
            cstr(file_name, "file_name")?,
            bytes,
            cstr(doctype, "doctype")?,
            cstr(docname, "docname")?,
            cstr(fieldname, "fieldname")?,
        ))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_download_file(
    c: *mut JssClient,
    file_url: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    run_bytes_op(out_data, out_len, || {
        let client = client_ref(c)?;
        block_on(client.inner.download_file(cstr(file_url, "file_url")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_download_pdf_kartu_piutang(
    c: *mut JssClient,
    doctype: *const c_char,
    name: *const c_char,
    format: *const c_char,
    no_letterhead: i32,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    run_bytes_op(out_data, out_len, || {
        let client = client_ref(c)?;
        block_on(client.inner.download_pdf_kartu_piutang(
            cstr(doctype, "doctype")?,
            cstr(name, "name")?,
            cstr(format, "format")?,
            no_letterhead != 0,
        ))
    })
}

// ───────────────────────── reports & search ────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_run_report(
    c: *mut JssClient,
    report_name: *const c_char,
    filters_json: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let v: serde_json::Value = serde_json::from_str(cstr(filters_json, "filters_json")?)
            .map_err(|e| JssError::Parse(format!("filters_json: {e}")))?;
        block_on(
            client
                .inner
                .run_report(cstr(report_name, "report_name")?, v),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_global_search(
    c: *mut JssClient,
    query: *const c_char,
    limit: u32,
    doctype: *const c_char,
    out_body: *mut *mut c_char,
) -> i32 {
    run_string_op(out_body, || {
        let client = client_ref(c)?;
        let q = cstr(query, "query")?;
        let dt = opt_cstr(doctype, "doctype")?;
        block_on(client.inner.global_search(q, limit, dt))
    })
}

// ───────────────────────── boot info getters ───────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_boot_sitename(c: *mut JssClient, out: *mut *mut c_char) -> i32 {
    run_string_op(out, || {
        let client = client_ref(c)?;
        let boot = client.inner.boot().ok_or(JssError::NotAuthenticated)?;
        Ok(boot.sitename.clone())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_boot_user_name(
    c: *mut JssClient,
    out: *mut *mut c_char,
) -> i32 {
    run_string_op(out, || {
        let client = client_ref(c)?;
        let boot = client.inner.boot().ok_or(JssError::NotAuthenticated)?;
        Ok(boot.user.name.clone())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_boot_user_full_name(
    c: *mut JssClient,
    out: *mut *mut c_char,
) -> i32 {
    run_string_op(out, || {
        let client = client_ref(c)?;
        let boot = client.inner.boot().ok_or(JssError::NotAuthenticated)?;
        Ok(boot.user.full_name.clone())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_boot_user_roles(
    c: *mut JssClient,
    out_json: *mut *mut c_char,
) -> i32 {
    run_string_op(out_json, || {
        let client = client_ref(c)?;
        let boot = client.inner.boot().ok_or(JssError::NotAuthenticated)?;
        serde_json::to_string(&boot.user.roles).map_err(|e| JssError::Parse(e.to_string()))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_accessible_doctypes(
    c: *mut JssClient,
    out_json: *mut *mut c_char,
) -> i32 {
    run_string_op(out_json, || {
        let client = client_ref(c)?;
        serde_json::to_string(&client.inner.accessible_doctypes())
            .map_err(|e| JssError::Parse(e.to_string()))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_is_developer_mode(c: *mut JssClient) -> i32 {
    run_bool_op(|| Ok(client_ref(c)?.inner.is_developer_mode()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_is_read_only(c: *mut JssClient) -> i32 {
    run_bool_op(|| Ok(client_ref(c)?.inner.is_read_only()))
}

// ───────────────────────── permission checks ───────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_can_read(c: *mut JssClient, doctype: *const c_char) -> i32 {
    run_bool_op(|| {
        let client = client_ref(c)?;
        Ok(client.inner.can_read(cstr(doctype, "doctype")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_can_write(c: *mut JssClient, doctype: *const c_char) -> i32 {
    run_bool_op(|| {
        let client = client_ref(c)?;
        Ok(client.inner.can_write(cstr(doctype, "doctype")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_can_create(c: *mut JssClient, doctype: *const c_char) -> i32 {
    run_bool_op(|| {
        let client = client_ref(c)?;
        Ok(client.inner.can_create(cstr(doctype, "doctype")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_can_submit(c: *mut JssClient, doctype: *const c_char) -> i32 {
    run_bool_op(|| {
        let client = client_ref(c)?;
        Ok(client.inner.can_submit(cstr(doctype, "doctype")?))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_client_can_delete(c: *mut JssClient, doctype: *const c_char) -> i32 {
    run_bool_op(|| {
        let client = client_ref(c)?;
        Ok(client.inner.can_delete(cstr(doctype, "doctype")?))
    })
}

// ───────────────────────── memory free ─────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(CString::from_raw(s));
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jss_bytes_free(p: *mut u8, len: usize) {
    if p.is_null() || len == 0 {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let slice = std::ptr::slice_from_raw_parts_mut(p, len);
        drop(Box::from_raw(slice));
    }));
}
