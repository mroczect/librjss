use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use tokio::runtime::Runtime;

use crate::api::AuthManager;
use crate::handler::config::AuthConfig;
use crate::handler::error::AuthError;
use crate::handler::types::{
    Credentials, HttpResponse, SessionId, SessionInfo, SessionStore, UserInfo, UserProvider,
};

static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("failed to create Tokio runtime"));

pub struct FfiAuthConfig(AuthConfig);
pub struct FfiAuthManager(AuthManager);
pub struct FfiHttpResponse(HttpResponse);
pub struct FfiSessionId(SessionId);
pub struct FfiSessionInfo(SessionInfo);
pub struct FfiUserInfo(UserInfo);

pub type AuthCallback = unsafe extern "C" fn(
    user_data: *mut std::ffi::c_void,
    username: *const c_char,
    password: *const c_char,
    out_user_id: *mut *mut c_char,
    out_extra_json: *mut *mut c_char,
    out_error_code: *mut i32,
) -> i32;

pub type GetUserCallback = unsafe extern "C" fn(
    user_data: *mut std::ffi::c_void,
    user_id: *const c_char,
    out_user_id: *mut *mut c_char,
    out_extra_json: *mut *mut c_char,
    out_error_code: *mut i32,
) -> i32;

pub type SessionSaveCallback = unsafe extern "C" fn(
    store_data: *mut std::ffi::c_void,
    session_id: *const c_char,
    user_id: *const c_char,
    data_json: *const c_char,
    issued_at_unix: i64,
    expires_at_unix: i64,
    idle_deadline_unix: i64,
) -> i32;

pub type SessionLoadCallback = unsafe extern "C" fn(
    store_data: *mut std::ffi::c_void,
    session_id: *const c_char,
    out_user_id: *mut *mut c_char,
    out_data_json: *mut *mut c_char,
    out_issued_at: *mut i64,
    out_expires_at: *mut i64,
    out_idle_deadline: *mut i64,
    out_error_code: *mut i32,
) -> i32;

pub type SessionDeleteCallback =
    unsafe extern "C" fn(store_data: *mut std::ffi::c_void, session_id: *const c_char) -> i32;

pub type SessionCleanupCallback = unsafe extern "C" fn(store_data: *mut std::ffi::c_void) -> i32;

struct CUserProvider {
    user_data: *mut std::ffi::c_void,
    auth_cb: Option<AuthCallback>,
    get_user_cb: Option<GetUserCallback>,
}

unsafe impl Send for CUserProvider {}
unsafe impl Sync for CUserProvider {}

#[async_trait]
impl UserProvider for CUserProvider {
    async fn authenticate(&self, credentials: &Credentials) -> Result<UserInfo, AuthError> {
        let auth_cb = self.auth_cb.expect("auth callback not set");
        let username = CString::new(credentials.username.as_str()).unwrap();
        let password = CString::new(credentials.password.as_str()).unwrap();
        let mut out_user_id: *mut c_char = ptr::null_mut();
        let mut out_extra_json: *mut c_char = ptr::null_mut();
        let mut err_code: i32 = 0;
        let ret = unsafe {
            auth_cb(
                self.user_data,
                username.as_ptr(),
                password.as_ptr(),
                &mut out_user_id,
                &mut out_extra_json,
                &mut err_code,
            )
        };
        if ret != 0 {
            return Err(map_error_code(err_code));
        }
        let user_id = unsafe { CStr::from_ptr(out_user_id) }
            .to_str()
            .unwrap()
            .to_owned();
        let extra_str = unsafe { CStr::from_ptr(out_extra_json) }.to_str().unwrap();
        let extra: serde_json::Value =
            serde_json::from_str(extra_str).unwrap_or(serde_json::Value::Null);
        unsafe {
            rjss_free_string(out_user_id);
            rjss_free_string(out_extra_json);
        }
        Ok(UserInfo { user_id, extra })
    }

    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserInfo>, AuthError> {
        let get_user_cb = self.get_user_cb.expect("get_user callback not set");
        let user_id_c = CString::new(user_id).unwrap();
        let mut out_user_id: *mut c_char = ptr::null_mut();
        let mut out_extra_json: *mut c_char = ptr::null_mut();
        let mut err_code: i32 = 0;
        let ret = unsafe {
            get_user_cb(
                self.user_data,
                user_id_c.as_ptr(),
                &mut out_user_id,
                &mut out_extra_json,
                &mut err_code,
            )
        };
        if ret != 0 {
            return Err(map_error_code(err_code));
        }
        if out_user_id.is_null() {
            return Ok(None);
        }
        let id = unsafe { CStr::from_ptr(out_user_id) }
            .to_str()
            .unwrap()
            .to_owned();
        let extra_str = unsafe { CStr::from_ptr(out_extra_json) }.to_str().unwrap();
        let extra: serde_json::Value =
            serde_json::from_str(extra_str).unwrap_or(serde_json::Value::Null);
        unsafe {
            rjss_free_string(out_user_id);
            rjss_free_string(out_extra_json);
        }
        Ok(Some(UserInfo { user_id: id, extra }))
    }
}

struct CSessionStore {
    store_data: *mut std::ffi::c_void,
    save_cb: Option<SessionSaveCallback>,
    load_cb: Option<SessionLoadCallback>,
    delete_cb: Option<SessionDeleteCallback>,
    cleanup_cb: Option<SessionCleanupCallback>,
}

unsafe impl Send for CSessionStore {}
unsafe impl Sync for CSessionStore {}

#[async_trait]
impl SessionStore for CSessionStore {
    async fn save(&self, id: &SessionId, info: &SessionInfo) -> Result<(), AuthError> {
        let save_cb = self.save_cb.expect("save callback not set");
        let sid_c = CString::new(id.as_str()).unwrap();
        let uid_c = CString::new(info.user_id.as_str()).unwrap();
        let data_json_c = CString::new(info.data.to_string()).unwrap();
        let idle_dl = info.idle_deadline.map(|t| t.unix_timestamp()).unwrap_or(-1);
        let ret = unsafe {
            save_cb(
                self.store_data,
                sid_c.as_ptr(),
                uid_c.as_ptr(),
                data_json_c.as_ptr(),
                info.issued_at.unix_timestamp(),
                info.expires_at.unix_timestamp(),
                idle_dl,
            )
        };
        if ret != 0 {
            return Err(AuthError::Internal("session save failed".into()));
        }
        Ok(())
    }

    async fn load(&self, id: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
        let load_cb = self.load_cb.expect("load callback not set");
        let sid_c = CString::new(id.as_str()).unwrap();
        let mut out_uid: *mut c_char = ptr::null_mut();
        let mut out_data: *mut c_char = ptr::null_mut();
        let mut issued: i64 = 0;
        let mut expires: i64 = 0;
        let mut idle: i64 = 0;
        let mut err_code: i32 = 0;
        let ret = unsafe {
            load_cb(
                self.store_data,
                sid_c.as_ptr(),
                &mut out_uid,
                &mut out_data,
                &mut issued,
                &mut expires,
                &mut idle,
                &mut err_code,
            )
        };
        if ret != 0 {
            return Err(map_error_code(err_code));
        }
        if out_uid.is_null() {
            return Ok(None);
        }
        let uid = unsafe { CStr::from_ptr(out_uid) }
            .to_str()
            .unwrap()
            .to_owned();
        let data_str = unsafe { CStr::from_ptr(out_data) }.to_str().unwrap();
        let data: serde_json::Value =
            serde_json::from_str(data_str).unwrap_or(serde_json::Value::Null);
        let issued_at = time::OffsetDateTime::from_unix_timestamp(issued)
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        let expires_at = time::OffsetDateTime::from_unix_timestamp(expires)
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        let idle_deadline = if idle == -1 {
            None
        } else {
            Some(
                time::OffsetDateTime::from_unix_timestamp(idle)
                    .map_err(|e| AuthError::Internal(e.to_string()))?,
            )
        };
        unsafe {
            rjss_free_string(out_uid);
            rjss_free_string(out_data);
        }
        Ok(Some(SessionInfo {
            user_id: uid,
            data,
            issued_at,
            expires_at,
            idle_deadline,
        }))
    }

    async fn delete(&self, id: &SessionId) -> Result<(), AuthError> {
        let delete_cb = self.delete_cb.expect("delete callback not set");
        let sid_c = CString::new(id.as_str()).unwrap();
        let ret = unsafe { delete_cb(self.store_data, sid_c.as_ptr()) };
        if ret != 0 {
            return Err(AuthError::Internal("session delete failed".into()));
        }
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), AuthError> {
        if let Some(cb) = self.cleanup_cb {
            let ret = unsafe { cb(self.store_data) };
            if ret != 0 {
                return Err(AuthError::Internal("session cleanup failed".into()));
            }
        }
        Ok(())
    }
}

fn map_error_code(code: i32) -> AuthError {
    match code {
        1 => AuthError::Config("callback config error".into()),
        2 => AuthError::InvalidCredentials,
        3 => AuthError::AccountLocked {
            until: "unknown".into(),
        },
        4 => AuthError::SessionExpired,
        5 => AuthError::SessionNotFound,
        6 => AuthError::Internal("callback internal error".into()),
        7 => AuthError::Serialization("callback serialization error".into()),
        _ => AuthError::Internal("unknown callback error".into()),
    }
}

#[allow(dead_code)]
fn auth_error_to_code(err: &AuthError) -> i32 {
    match err {
        AuthError::Config(_) => 1,
        AuthError::InvalidCredentials => 2,
        AuthError::AccountLocked { .. } => 3,
        AuthError::SessionExpired => 4,
        AuthError::SessionNotFound => 5,
        AuthError::Internal(_) => 6,
        AuthError::Serialization(_) => 7,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_alloc_string(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(s) };
    CString::new(c_str.to_bytes()).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rjss_config_new_default() -> *mut FfiAuthConfig {
    Box::into_raw(Box::new(FfiAuthConfig(AuthConfig::default())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_config_from_env(out_error: *mut *mut c_char) -> *mut FfiAuthConfig {
    match AuthConfig::from_env() {
        Ok(cfg) => Box::into_raw(Box::new(FfiAuthConfig(cfg))),
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_config_set_cookie_name(
    config: *mut FfiAuthConfig,
    name: *const c_char,
) {
    if config.is_null() || name.is_null() {
        return;
    }
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    let cfg = unsafe { &mut *config };
    cfg.0.cookie_name = name;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_config_set_session_lifetime(
    config: *mut FfiAuthConfig,
    seconds: i64,
) {
    if config.is_null() || seconds <= 0 {
        return;
    }
    let cfg = unsafe { &mut *config };
    cfg.0.session_lifetime = time::Duration::seconds(seconds);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_config_free(config: *mut FfiAuthConfig) {
    if config.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(config));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_auth_manager_new(
    config: *mut FfiAuthConfig,
    user_data: *mut std::ffi::c_void,
    auth_cb: Option<AuthCallback>,
    get_user_cb: Option<GetUserCallback>,
    store_data: *mut std::ffi::c_void,
    save_cb: Option<SessionSaveCallback>,
    load_cb: Option<SessionLoadCallback>,
    delete_cb: Option<SessionDeleteCallback>,
    cleanup_cb: Option<SessionCleanupCallback>,
) -> *mut FfiAuthManager {
    if config.is_null()
        || auth_cb.is_none()
        || get_user_cb.is_none()
        || save_cb.is_none()
        || load_cb.is_none()
        || delete_cb.is_none()
    {
        if !config.is_null() {
            unsafe { rjss_config_free(config) };
        }
        return ptr::null_mut();
    }
    let cfg = unsafe { Box::from_raw(config) }.0;
    let user_provider = Arc::new(CUserProvider {
        user_data,
        auth_cb,
        get_user_cb,
    });
    let session_store = Arc::new(CSessionStore {
        store_data,
        save_cb,
        load_cb,
        delete_cb,
        cleanup_cb,
    });
    let manager = AuthManager::new(cfg, user_provider, session_store);
    Box::into_raw(Box::new(FfiAuthManager(manager)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_auth_manager_new_with_memory_store(
    config: *mut FfiAuthConfig,
    user_data: *mut std::ffi::c_void,
    auth_cb: Option<AuthCallback>,
    get_user_cb: Option<GetUserCallback>,
) -> *mut FfiAuthManager {
    if config.is_null() || auth_cb.is_none() || get_user_cb.is_none() {
        if !config.is_null() {
            unsafe { rjss_config_free(config) };
        }
        return ptr::null_mut();
    }
    let cfg = unsafe { Box::from_raw(config) }.0;
    let user_provider = Arc::new(CUserProvider {
        user_data,
        auth_cb,
        get_user_cb,
    });
    let session_store = Arc::new(crate::handler::session_store::MemorySessionStore::new());
    let manager = AuthManager::new(cfg, user_provider, session_store);
    Box::into_raw(Box::new(FfiAuthManager(manager)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_auth_manager_free(manager: *mut FfiAuthManager) {
    if manager.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(manager));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_login(
    manager: *const FfiAuthManager,
    username: *const c_char,
    password: *const c_char,
    out_error: *mut *mut c_char,
) -> *mut FfiHttpResponse {
    if manager.is_null() || username.is_null() || password.is_null() {
        if !out_error.is_null() {
            unsafe {
                *out_error = CString::new("null arguments").unwrap().into_raw();
            }
        }
        return ptr::null_mut();
    }
    let mgr = &unsafe { &*manager }.0;
    let username = unsafe { CStr::from_ptr(username) }
        .to_string_lossy()
        .into_owned();
    let password = unsafe { CStr::from_ptr(password) }
        .to_string_lossy()
        .into_owned();
    let creds = Credentials { username, password };
    let result = RUNTIME.block_on(mgr.login(creds));
    match result {
        Ok(resp) => Box::into_raw(Box::new(FfiHttpResponse(resp))),
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_logout(
    manager: *const FfiAuthManager,
    session_id: *const c_char,
    out_error: *mut *mut c_char,
) -> *mut FfiHttpResponse {
    if manager.is_null() {
        if !out_error.is_null() {
            unsafe {
                *out_error = CString::new("null manager").unwrap().into_raw();
            }
        }
        return ptr::null_mut();
    }
    let sid = if session_id.is_null() {
        None
    } else {
        let s = unsafe { CStr::from_ptr(session_id) }
            .to_string_lossy()
            .into_owned();
        Some(SessionId::new(s))
    };
    let mgr = &unsafe { &*manager }.0;
    let result = RUNTIME.block_on(mgr.logout(sid.as_ref()));
    match result {
        Ok(resp) => Box::into_raw(Box::new(FfiHttpResponse(resp))),
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_validate_session(
    manager: *const FfiAuthManager,
    session_id: *const c_char,
    out_error: *mut *mut c_char,
) -> *mut FfiSessionInfo {
    if manager.is_null() || session_id.is_null() {
        if !out_error.is_null() {
            unsafe {
                *out_error = CString::new("null arguments").unwrap().into_raw();
            }
        }
        return ptr::null_mut();
    }
    let sid = SessionId::new(
        unsafe { CStr::from_ptr(session_id) }
            .to_string_lossy()
            .into_owned(),
    );
    let mgr = &unsafe { &*manager }.0;
    let result = RUNTIME.block_on(mgr.validate_session(&sid));
    match result {
        Ok(info) => Box::into_raw(Box::new(FfiSessionInfo(info))),
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_create_session(
    manager: *const FfiAuthManager,
    user_id: *const c_char,
    extra_json: *const c_char,
    out_session_id: *mut *mut c_char,
    out_session_info: *mut *mut FfiSessionInfo,
    out_error: *mut *mut c_char,
) -> i32 {
    if manager.is_null()
        || user_id.is_null()
        || out_session_id.is_null()
        || out_session_info.is_null()
    {
        if !out_error.is_null() {
            unsafe {
                *out_error = CString::new("null arguments").unwrap().into_raw();
            }
        }
        return -1;
    }
    let uid = unsafe { CStr::from_ptr(user_id) }
        .to_string_lossy()
        .into_owned();
    let extra_str = if extra_json.is_null() {
        "null"
    } else {
        unsafe { CStr::from_ptr(extra_json) }
            .to_str()
            .unwrap_or("null")
    };
    let extra: serde_json::Value =
        serde_json::from_str(extra_str).unwrap_or(serde_json::Value::Null);
    let user_info = UserInfo {
        user_id: uid,
        extra,
    };
    let mgr = &unsafe { &*manager }.0;
    let result = RUNTIME.block_on(mgr.create_session(&user_info));
    match result {
        Ok((sid, info)) => {
            unsafe {
                *out_session_id = CString::new(sid.to_string()).unwrap().into_raw();
                *out_session_info = Box::into_raw(Box::new(FfiSessionInfo(info)));
            }
            0
        }
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_destroy_session(
    manager: *const FfiAuthManager,
    session_id: *const c_char,
    out_error: *mut *mut c_char,
) -> i32 {
    if manager.is_null() || session_id.is_null() {
        if !out_error.is_null() {
            unsafe {
                *out_error = CString::new("null arguments").unwrap().into_raw();
            }
        }
        return -1;
    }
    let sid = SessionId::new(
        unsafe { CStr::from_ptr(session_id) }
            .to_string_lossy()
            .into_owned(),
    );
    let mgr = &unsafe { &*manager }.0;
    match RUNTIME.block_on(mgr.destroy_session(&sid)) {
        Ok(()) => 0,
        Err(e) => {
            if !out_error.is_null() {
                unsafe {
                    *out_error = CString::new(e.to_string()).unwrap().into_raw();
                }
            }
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_response_status(response: *const FfiHttpResponse) -> u16 {
    if response.is_null() {
        return 0;
    }
    unsafe { &*response }.0.status.as_u16()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_response_body(response: *const FfiHttpResponse) -> *mut c_char {
    if response.is_null() {
        return ptr::null_mut();
    }
    CString::new(unsafe { &*response }.0.body.clone())
        .unwrap()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_response_header_count(response: *const FfiHttpResponse) -> usize {
    if response.is_null() {
        return 0;
    }
    unsafe { &*response }.0.headers.len()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_response_header_at(
    response: *const FfiHttpResponse,
    index: usize,
    out_key: *mut *mut c_char,
    out_value: *mut *mut c_char,
) -> i32 {
    if response.is_null() || out_key.is_null() || out_value.is_null() {
        return -1;
    }
    let resp = &unsafe { &*response }.0;
    if index >= resp.headers.len() {
        return -1;
    }
    let (key, value) = &resp.headers[index];
    unsafe {
        *out_key = CString::new(key.clone()).unwrap().into_raw();
        *out_value = CString::new(value.clone()).unwrap().into_raw();
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_response_free(response: *mut FfiHttpResponse) {
    if response.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(response));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_info_user_id(info: *const FfiSessionInfo) -> *mut c_char {
    if info.is_null() {
        return ptr::null_mut();
    }
    CString::new(unsafe { &*info }.0.user_id.clone())
        .unwrap()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_info_data(info: *const FfiSessionInfo) -> *mut c_char {
    if info.is_null() {
        return ptr::null_mut();
    }
    CString::new(unsafe { &*info }.0.data.to_string())
        .unwrap()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_info_free(info: *mut FfiSessionInfo) {
    if info.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(info));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_id_new(id_str: *const c_char) -> *mut FfiSessionId {
    if id_str.is_null() {
        return ptr::null_mut();
    }
    let s = unsafe { CStr::from_ptr(id_str) }
        .to_string_lossy()
        .into_owned();
    Box::into_raw(Box::new(FfiSessionId(SessionId::new(s))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_id_str(session_id: *const FfiSessionId) -> *mut c_char {
    if session_id.is_null() {
        return ptr::null_mut();
    }
    CString::new(unsafe { &*session_id }.0.to_string())
        .unwrap()
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_session_id_free(session_id: *mut FfiSessionId) {
    if session_id.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(session_id));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rjss_error_message(code: i32) -> *mut c_char {
    let err = map_error_code(code);
    CString::new(err.to_string()).unwrap().into_raw()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rjss_user_info_user_id(info: *const FfiUserInfo) -> *mut c_char {
    if info.is_null() { return ptr::null_mut(); }
    CString::new(unsafe { &*info }.0.user_id.clone()).unwrap().into_raw()
}