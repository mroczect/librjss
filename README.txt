librjss -- readme
=================

async rust client for frappe / erpnext backends.

session auth and token auth. automatic re-login. retry with backoff.
typed builders. read-only guard. error type mirrors frappe json.

also ships a c ffi (librjss-ffi) for c, c++, python, go, etc.

rust 1.88+ edition 2024. license mit.
repo https://github.com/mroczect/librjss


install
-------

[dependencies]
librjss    = { git = "https://github.com/mroczect/librjss", branch = "master" }
tokio      = { version = "1", features = ["full"] }
secrecy    = "0.10"
serde      = { version = "1", features = ["derive"] }
serde_json = "1"


quick start
-----------

use librjss::{AuthMode, ClientConfig, RjssClient};
use secrecy::SecretString;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ClientConfig {
        base_url: "https://erp.example.com".parse()?,
        auth_mode: AuthMode::Session {
            email:    SecretString::new("user@example.com".into()),
            password: SecretString::new("hunter2".into()),
        },
        expected_sitename: Some("erp.example.com".into()),
        required_roles:    vec!["System Manager".into()],
        timeout_secs:      30,
        max_retries:       3,
        user_agent:        "my-app/1.0".into(),
        insecure_ssl:      false,
        readonly_guard:    true,
    };

    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    let raw = client
        .doctype("Customer")
        .fields(vec!["name", "customer_name", "territory"])
        .filter("disabled", "=", "0")
        .order_by("creation desc")
        .limit(10)
        .execute_raw()
        .await?;

    println!("{raw}");
    Ok(())
}


config
------

struct ClientConfig {
    base_url:          reqwest::Url,
    auth_mode:         AuthMode,
    expected_sitename: Option<String>,
    required_roles:    Vec<String>,
    timeout_secs:      u64,
    max_retries:       u32,
    user_agent:        String,
    insecure_ssl:      bool,
    readonly_guard:    bool,
}

field notes:

  base_url           must include scheme + host. no trailing slash needed.
  auth_mode          Session { email, password } or Token { api_key, api_secret }.
  expected_sitename  if set, login fails on mismatch with frappe.boot.sitename.
  required_roles     if non-empty, login fails unless user has at least one.
  timeout_secs       per-request timeout.
  max_retries        retries for transient errors (5xx, 429, network).
  user_agent         sent on every request.
  insecure_ssl       accept self-signed / invalid tls certs.
  readonly_guard     enables the read-only guard. see guard section.

config is validated in RjssClient::new.
https required unless insecure_ssl = true.
schemes data, javascript, vbscript rejected.


environment
-----------

ClientConfig::from_env() reads:

  JSS_BASE_URL / JSS_URL          required
  JSS_EMAIL / JSS_USR             -
  JSS_PASSWORD / JSS_PWD          -
  JSS_TOKEN_KEY                   -
  JSS_TOKEN_SECRET                -
  JSS_EXPECTED_SITENAME           -
  JSS_REQUIRED_ROLES              comma list
  JSS_TIMEOUT_SECS                30
  JSS_MAX_RETRIES                 3
  JSS_USER_AGENT                  librjss/2.3.0
  JSS_INSECURE_SSL                false
  JSS_READONLY_GUARD              true

if JSS_TOKEN_KEY set, token mode used.
otherwise email/password required.
from_env returns JssError::Config if neither present.

    let cfg = ClientConfig::from_env()?;
    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;


auth
----

session
  POST /api/method/login with usr + pwd.
  on success library fetches /app, parses frappe.boot, extracts
  csrf_token, sitename, user info, role list. cookie jar handled
  by reqwest.

      AuthMode::Session {
          email:    SecretString::new("user@example.com".into()),
          password: SecretString::new("hunter2".into()),
      }

  session can expire server-side. use ensure_session() to
  re-validate and re-login transparently.

token
  sends Authorization: token <api_key>:<api_secret> on every
  request. no login roundtrip. no session. permissions inherited
  from the user that owns the key.

      AuthMode::Token {
          api_key:    "abcd1234".into(),
          api_secret: SecretString::new("efgh5678".into()),
      }

re-login
      client.ensure_session().await?;

  internally calls frappe.auth.get_logged_user. on 401 drops
  session, re-logs in with cached credentials, retries up to 3 times.


client api
----------

all request methods return Result<String, JssError> (raw body).
serialization / deserialization on you, except helpers below.

http verbs
    client.authenticated_get("/api/resource/ToDo").await?;
    client.authenticated_post("/api/resource/ToDo", r#"{"status":"Open"}"#).await?;
    client.authenticated_put("/api/resource/ToDo/TODO-0001", r#"{"status":"Closed"}"#).await?;
    client.authenticated_delete("/api/resource/ToDo/TODO-0001").await?;

    client.post_form(
        "/api/method/frappe.client.get_count",
        &[("doctype", "Customer")],
    ).await?;

  each attaches correct auth header, adds X-Frappe-CSRF-Token
  when session mode, retries transient errors with exponential backoff.

single documents
    let raw: String = client.get_doc("Customer", "CUST-0001").await?;
    let json: serde_json::Value = client.get_doc_json("Customer", "CUST-0001").await?;
    let created = client.create_doc("Customer", &new_customer).await?;
    let updated = client.update_doc("Customer", "CUST-0001", &patch).await?;
    client.delete_doc("Customer", "CUST-9999").await?;

list queries -- ResourceBuilder
    let rows: serde_json::Value = client
        .doctype("Sales Invoice")
        .filter("status", "=", "Unpaid")
        .filter("customer", "=", "ACME")
        .fields(vec!["name", "customer", "grand_total", "posting_date"])
        .order_by("posting_date desc")
        .limit(50)
        .limit_start(0)
        .execute()
        .await?;

  supported filter operators: = != > < >= <= like in "not in"
  is (set / not set) between.

  execute_raw() returns String. execute::<T>() returns typed T.

pagination
    #[derive(serde::Deserialize)]
    struct Customer { name: String, customer_name: String }

    let all: Vec<Customer> = client
        .doctype("Customer")
        .filter("disabled", "=", "0")
        .fields(vec!["name", "customer_name"])
        .all()
        .await?;

  .all() pages at 200 rows, appends until a page returns fewer
  rows than requested, then deserializes aggregate into Vec<T>.

reports -- ReportBuilder
    #[derive(serde::Deserialize)]
    struct Row { name: String, total: f64 }

    let rows: Vec<Row> = client
        .report("Sales Analytics")
        .add_filter("from_date", "2026-01-01")
        .add_filter("to_date",   "2026-06-30")
        .run()
        .await?;

    let raw = client.report("Sales Analytics").run_raw().await?;

  note: script reports can have side effects. guard blocks
  query_report.run by default. whitelist explicitly if audited.

files
    let bytes = std::fs::read("invoice.pdf")?;
    client.upload_file("invoice.pdf", bytes, "Sales Invoice", "SINV-0001", "attach").await?;

    let bytes = client.download_file("/files/invoice.pdf").await?;
    client.download_file_to_path("/files/invoice.pdf", std::path::Path::new("local.pdf")).await?;

    let pdf = client
        .download_pdf_kartu_piutang(
            "Master Data Nasabah",
            "ODR-00001",
            "Form Rincian Sisa Piutang Nasabah",
            false,
        )
        .await?;

  file names and urls validated against .., absolute urls, path traversal.

workflow
    let transitions = client.workflow_transitions("Sales Order", "SO-0001").await?;
    client.apply_workflow("Sales Order", "SO-0001", "Approve").await?;
    client.transition_to_state("Sales Order", "SO-0001", "Approved").await?;

  transition_to_state fetches transitions, finds one whose
  next_state matches target, applies its action.

search
    client.global_search("acme", 10, Some("Customer")).await?;
    client.search_link("ac", "Customer", "Sales Invoice", 10).await?;

frappe convenience
    client.get_count("Customer", "[]", "[]", false).await?;
    client.get_list("Customer", fields_json, filters_json, "[]", "creation desc", 0, 50, "List", "", false).await?;
    client.get_list_settings("Customer").await?;
    client.get_doctype_meta("Customer").await?;
    client.get_notifications(20).await?;
    client.get_events("2026-01-01", "2026-01-31").await?;
    client.get_desktop_page("Sales", "Sales", true).await?;
    client.get_lazy_child_rows("ODR-00001", "items").await?;
    client.run_report("Sales Analytics", serde_json::json!({"from_date":"2026-01-01"})).await?;
    client.save_user_settings("Customer", user_settings).await?;
    client.get_transitions(doc_json).await?;

    use std::collections::HashMap;
    let mut args = HashMap::new();
    args.insert("doctype".into(), "Sales Invoice".into());
    args.insert("docname".into(), "SINV-0001".into());
    client.call_method("frappe.email.send", Some(args)).await?;

  call_method stringifies every json value into HashMap<String, String>
  that frappe expects on form-encoded endpoints.

boot data
  after successful login frappe.boot parsed and cached.

    let boot = client.boot().unwrap();
    println!("{} <{}>", boot.user.full_name, boot.user.email);
    println!("roles: {:?}", boot.user.roles);
    println!("versions: {:?}", boot.versions);

  convenience accessors:
    client.user_info_map();
    client.sidebar_pages();
    client.navbar_settings();
    client.versions();
    client.lang_dict();
    client.frequent_links();
    client.is_developer_mode();
    client.is_read_only();

permissions
  read-only checks against cached boot data. no network call.

    client.can_read("Customer");
    client.can_write("Sales Invoice");
    client.can_create("ToDo");
    client.can_submit("Sales Order");
    client.can_delete("File");
    client.accessible_doctypes();

misc
    client.base_url();
    client.trace_id();
    client.session_info();
    client.is_readonly_guard_on();


read-only guard
---------------

blocks POST / PUT / DELETE to any path not in explicit whitelist.
motivation: analysis tools and dev scripts shouldn't mutate by
accident.

whitelist lives in src/client/guard.rs and covers read-only frappe
methods that happen to use POST because of form encoding:

    /api/method/login
    /api/method/logout
    /api/method/frappe.sessions.get_csrf_token
    /api/method/frappe.auth.get_logged_user
    /api/method/frappe.client.get_count
    /api/method/frappe.client.get_list
    /api/method/frappe.client.get_value
    /api/method/frappe.client.get
    /api/method/frappe.client.validate_link
    /api/method/frappe.desk.reportview.get_list
    /api/method/frappe.desk.reportview.get_count
    /api/method/frappe.desk.search.search_link
    /api/method/frappe.desk.form.load.getdoc
    /api/method/frappe.desk.form.load.getdoctype
    /api/method/frappe.desk.desktop.get_desktop_page
    /api/method/frappe.utils.global_search.search
    /api/method/frappe.model.workflow.get_transitions

extend by editing ReadOnlyGuard::new(). add only paths audited as
read-only.

disable per client:
    ClientConfig { readonly_guard: false, ..cfg }

or globally:
    JSS_READONLY_GUARD=false

blocked request never touches network and returns
JssError::Validation with path and reason.


errors
------

enum JssError {
    Config(String),
    Validation(String),
    Network(reqwest::Error),
    Http { status: StatusCode, body: String },
    ApiError { exc_type: String, message: String, status: StatusCode },
    Auth(String),
    Csrf(String),
    Permission(String),
    SitenameMismatch { expected: String, actual: String },
    NotAuthenticated,
    RateLimited { retry_after: Option<u64> },
    Parse(String),
    Expired,
    Cancelled,
    FileOperation(String),
    Internal(String),
}

JssError::from_api_response(status, body) parses frappe json envelope:

    {
      "exc_type": "ValidationError",
      "_server_messages": "[\"{\\\"message\\\": \\\"Field X is required\\\"}\"]",
      "exc": "Traceback..."
    }

extracts exc_type and either _server_messages or exc.

JssError::status_code() maps each variant to logical http status.
useful when wrapping client in a service.

example:

    use librjss::JssError;

    match client.authenticated_get("/api/resource/ToDo").await {
        Ok(body) => println!("{body}"),

        Err(JssError::ApiError { exc_type, message, status }) =>
            eprintln!("[{status}] {exc_type}: {message}"),

        Err(JssError::RateLimited { retry_after }) => {
            let secs = retry_after.unwrap_or(5);
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        }

        Err(JssError::NotAuthenticated) => {
            client.ensure_session().await?;
        }

        Err(e) => eprintln!("{e} (status {})", e.status_code()),
    }


types
-----

AuthMode
    enum AuthMode {
        Session { email: SecretString, password: SecretString },
        Token   { api_key: String, api_secret: SecretString },
    }

SessionInfo
    struct SessionInfo {
        sid:        SecretString,
        csrf_token: SecretString,
        full_name:  Option<String>,
        sitename:   String,
        roles:      Vec<String>,
    }

FrappeBoot
  parsed from frappe.boot js object on /app. every field has a
  default so future frappe additions won't break deserialization.

    struct FrappeBoot {
        user: BootUser,
        sitename: String,
        csrf_token: String,
        sysdefaults: SysDefaults,
        app_logo_url: Option<String>,
        home_page: Option<String>,
        allow_modules: Vec<String>,
        can_select: Vec<String>,
        can_create: Vec<String>,
        can_write: Vec<String>,
        can_read: Vec<String>,
        can_submit: Vec<String>,
        can_cancel: Vec<String>,
        can_delete: Vec<String>,
        can_get_report: Vec<String>,
        all_reports: HashMap<String, ReportMeta>,
        module_wise_workspaces: HashMap<String, Vec<String>>,
        dashboards: Vec<DashboardMeta>,
        single_types: Vec<String>,
        calendars: Vec<String>,
        treeviews: Vec<String>,
        module_app: HashMap<String, String>,
        app_data: Vec<AppData>,
        user_info: HashMap<String, BootUserInfo>,
        sidebar_pages: SidebarPages,
        navbar_settings: Option<NavbarSettings>,
        versions: HashMap<String, String>,
        lang_dict: HashMap<String, String>,
        lang: Option<String>,
        page_info: HashMap<String, PageInfo>,
        frequently_visited_links: Vec<FrequentLink>,
        developer_mode: i32,
        read_only: bool,
        socketio_port: Option<u16>,
        desk_settings: HashMap<String, serde_json::Value>,
        desk_theme: Option<String>,
    }

  supporting types (BootUser, SysDefaults, ReportMeta, AppData,
  BootUserInfo, SidebarPages, NavbarSettings, FrequentLink,
  PageInfo) re-exported from librjss::.

WorkflowTransition
    struct WorkflowTransition {
        action: String,
        state: String,
        next_state: Option<String>,
        allowed: Option<String>,
        allow_self_approval: Option<i32>,
    }

ReportBuilder
    client.report(name: &str) -> ReportBuilder<'_>

    pub fn add_filter(self, field: &str, value: &str) -> Self;
    pub async fn run_raw(&self) -> Result<String, JssError>;
    pub async fn run<T: DeserializeOwned>(&self) -> Result<T, JssError>;

ResourceBuilder
    client.doctype(name: &str) -> ResourceBuilder<'_>

    pub fn filter(self, field: &str, operator: &str, value: &str) -> Self;
    pub fn fields(self, fields: Vec<&str>) -> Self;
    pub fn order_by(self, order: &str) -> Self;
    pub fn limit(self, limit: u32) -> Self;
    pub fn limit_start(self, start: u32) -> Self;

    pub async fn execute_raw(&self) -> Result<String, JssError>;
    pub async fn execute<T: DeserializeOwned>(&self) -> Result<T, JssError>;
    pub async fn all<T: DeserializeOwned>(&self) -> Result<Vec<T>, JssError>;

AuthEndpoints
  static url builders, usable without client instance.

    pub trait AuthEndpoints {
        fn login_url(base: &Url) -> Result<Url, JssError>;
        fn logout_url(base: &Url) -> Result<Url, JssError>;
        fn csrf_token_url(base: &Url) -> Result<Url, JssError>;
        fn get_logged_user_url(base: &Url) -> Result<Url, JssError>;
        fn app_page_url(base: &Url) -> Result<Url, JssError>;
    }

  implemented by RjssClient.


c ffi -- librjss-ffi
--------------------

workspace ships cdylib + staticlib + rlib exposing opaque client
handle to c and other languages.

cargo:

    [dependencies]
    librjss = { path = ".." }

    [lib]
    name = "librjss_ffi"
    crate-type = ["cdylib", "staticlib", "rlib"]

build:
    cargo build -p librjss-ffi --release

artifacts:
    target/release/liblibrjss_ffi.so   (or .dylib / .dll)
    target/release/liblibrjss_ffi.a
    librjss-ffi/include/librjss.h      (generated by cbindgen)

c usage:

    #include "librjss.h"
    #include <string.h>

    JssClientConfig cfg;
    memset(&cfg, 0, sizeof(cfg));
    cfg.base_url = "https://erp.example.com";
    cfg.auth_kind = "session";
    cfg.principal = "user@example.com";
    cfg.secret    = "hunter2";
    cfg.timeout_secs = 30;
    cfg.max_retries  = 3;

    JssClient *c = jss_client_new(&cfg);
    if (!c) {
        fprintf(stderr, "create failed: %s\n", jss_last_error());
        return 1;
    }

    if (jss_client_authenticate(c) != JSS_OK) {
        fprintf(stderr, "auth failed: %s\n", jss_last_error());
        jss_client_free(c);
        return 1;
    }

    char *body = NULL;
    if (jss_client_get(c, "/api/resource/ToDo?limit_page_length=5", &body) == JSS_OK) {
        puts(body);
        jss_string_free(body);
    }

    jss_client_free(c);

compile:
    cc -I include your_app.c \
       target/release/liblibrjss_ffi.a \
       -lpthread -ldl -lm \
       -o your_app

config struct:

    typedef struct JssClientConfig {
        const char *base_url;
        const char *auth_kind;        // "session" | "token"
        const char *principal;        // email | api_key
        const char *secret;           // password | api_secret
        const char *expected_sitename;
        uint32_t    flags;            // JSS_FLAG_*
        uint32_t    _reserved;
        uint64_t    timeout_secs;
        uint32_t    max_retries;
        uint32_t    _reserved2;
    } JssClientConfig;

flags:

    JSS_FLAG_INSECURE_SSL        accept invalid tls certs
    JSS_FLAG_NO_READONLY_GUARD   disable read-only guard

function list:

  lifecycle
    JssClient *jss_client_new(const JssClientConfig *cfg);
    void       jss_client_free(JssClient *c);
    const char *jss_last_error(void);   // thread-local, do not free
    const char *jss_version(void);      // static, do not free

  auth
    int32_t jss_client_authenticate(JssClient *c);
    int32_t jss_client_logout(JssClient *c);
    int32_t jss_client_ensure_session(JssClient *c);
    int32_t jss_client_is_authenticated(JssClient *c);  // 1 / 0 / <err>

  http
    int32_t jss_client_get(JssClient *c, const char *path, char **out_body);
    int32_t jss_client_post(JssClient *c, const char *path, const char *body_json, char **out_body);
    int32_t jss_client_put(JssClient *c, const char *path, const char *body_json, char **out_body);
    int32_t jss_client_delete(JssClient *c, const char *path, char **out_body);
    int32_t jss_client_post_form(JssClient *c, const char *path,
                                 const char *const *keys,
                                 const char *const *values,
                                 uintptr_t n_pairs,
                                 char **out_body);

  documents
    int32_t jss_client_get_doc(JssClient *c, const char *doctype, const char *name, char **out_body);
    int32_t jss_client_create_doc(JssClient *c, const char *doctype, const char *data_json, char **out_body);
    int32_t jss_client_update_doc(JssClient *c, const char *doctype, const char *name, const char *data_json, char **out_body);
    int32_t jss_client_delete_doc(JssClient *c, const char *doctype, const char *name, char **out_body);
    int32_t jss_client_call_method(JssClient *c, const char *method, const char *args_json, char **out_body);

  files
    int32_t jss_client_upload_file(JssClient *c,
                                   const char *file_name,
                                   const uint8_t *content, uintptr_t content_len,
                                   const char *doctype, const char *docname, const char *fieldname,
                                   char **out_body);
    int32_t jss_client_download_file(JssClient *c, const char *file_url,
                                     uint8_t **out_data, uintptr_t *out_len);
    int32_t jss_client_download_pdf_kartu_piutang(JssClient *c,
                                                  const char *doctype, const char *name,
                                                  const char *format, int32_t no_letterhead,
                                                  uint8_t **out_data, uintptr_t *out_len);

  reports and search
    int32_t jss_client_run_report(JssClient *c, const char *report_name, const char *filters_json, char **out_body);
    int32_t jss_client_global_search(JssClient *c, const char *query, uint32_t limit, const char *doctype, char **out_body);

  boot getters
    int32_t jss_client_boot_sitename(JssClient *c, char **out);
    int32_t jss_client_boot_user_name(JssClient *c, char **out);
    int32_t jss_client_boot_user_full_name(JssClient *c, char **out);
    int32_t jss_client_boot_user_roles(JssClient *c, char **out_json);
    int32_t jss_client_accessible_doctypes(JssClient *c, char **out_json);
    int32_t jss_client_is_developer_mode(JssClient *c);
    int32_t jss_client_is_read_only(JssClient *c);

  permissions
    int32_t jss_client_can_read(JssClient *c, const char *doctype);
    int32_t jss_client_can_write(JssClient *c, const char *doctype);
    int32_t jss_client_can_create(JssClient *c, const char *doctype);
    int32_t jss_client_can_submit(JssClient *c, const char *doctype);
    int32_t jss_client_can_delete(JssClient *c, const char *doctype);

  memory
    void jss_string_free(char *s);
    void jss_bytes_free(uint8_t *p, uintptr_t len);

every out_body / out_data returned with JSS_OK must be freed with
corresponding _free function. jss_last_error() is thread-local and
must not be freed.

error codes:

    JSS_OK                      0
    JSS_ERR_CONFIG             -1
    JSS_ERR_VALIDATION         -2
    JSS_ERR_NETWORK            -3
    JSS_ERR_HTTP               -4
    JSS_ERR_API                -5
    JSS_ERR_AUTH               -6
    JSS_ERR_CSRF               -7
    JSS_ERR_PERMISSION         -8
    JSS_ERR_SITENAME_MISMATCH  -9
    JSS_ERR_NOT_AUTHENTICATED -10
    JSS_ERR_RATE_LIMITED      -11
    JSS_ERR_PARSE             -12
    JSS_ERR_EXPIRED           -13
    JSS_ERR_CANCELLED         -14
    JSS_ERR_FILE_OPERATION    -15
    JSS_ERR_INTERNAL          -16
    JSS_ERR_NULL_POINTER      -17
    JSS_ERR_UTF8              -18
    JSS_ERR_PANIC             -19

panics inside rust code caught and reported as JSS_ERR_PANIC.
message available via jss_last_error().


development
-----------

mock server
  examples/jss_mock_server.rs emulates:
    POST /api/method/login
    GET  /app                (with synthetic frappe.boot)
    GET  /api/method/frappe.auth.get_logged_user
    POST /api/method/logout
    GET  /api/method/frappe.auth.get_csrf_token

    cargo run --example jss_mock_server

  then point client at http://127.0.0.1:8080 with JSS_INSECURE_SSL=true.

tests
    cargo test --lib                 unit tests, no server
    cargo test --test client_tests   integration, needs mock server
    cargo test --workspace           everything

checks before commit
    cargo fmt --all
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test  --workspace --all-targets --all-features

ffi smoke test
    cargo build -p librjss-ffi --release
    cd librjss-ffi && make run

  requires cc and env vars used by examples/hello.c.


security
--------

credentials and tokens wrapped in secrecy::SecretString, zeroed on
drop. exposed only at moment a request header is built.

logs include path, status, body hash. bodies, headers, secrets
never logged.

read-only guard runs before network call. blocked writes never reach
server.

insecure_ssl = true should be limited to local development against
self-signed certs.

prefer api keys with minimal roles over session auth for services.
token inherits every permission of its owning user.

rotate credentials if leaked. sessions can be invalidated from ui.


license
-------

mit. see LICENSE file.


see also
--------

  frappe framework docs   https://frappeframework.com/docs
  erpnext docs            https://docs.erpnext.com
  reqwest                 https://docs.rs/reqwest
  serde                   https://docs.rs/serde
  secrecy                 https://docs.rs/secrecy