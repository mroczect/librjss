# librjss

**A robust, async Rust client for Frappe / ERPNext / Juragan Subsystem (JSS) APIs.**

[![Crates.io](https://img.shields.io/badge/version-2.3.0-blue)](https://crates.io/crates/librjss)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange)](https://www.rust-lang.org)
[![CI](https://github.com/mroczect/librjss/actions/workflows/ci.yml/badge.svg)](https://github.com/mroczect/librjss/actions)

Production-grade Rust client for Frappe-based backends. Cookie session auth, token auth, automatic re-login, retry with exponential backoff, typed builders, a read-only safety guard, and a rich error model that mirrors Frappe's JSON responses.

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
  - [Environment Variables](#environment-variables)
  - [Programmatic Configuration](#programmatic-configuration)
- [Authentication](#authentication)
  - [Session Auth](#session-auth)
  - [Token Auth](#token-auth)
  - [Auto Re-login](#auto-re-login)
- [Usage Examples](#usage-examples)
  - [Basic HTTP Requests](#basic-http-requests)
  - [Working with DocTypes](#working-with-doctypes)
  - [Pagination](#pagination)
  - [Single Document CRUD](#single-document-crud)
  - [Reports](#reports)
  - [Files and PDFs](#files-and-pdfs)
  - [Workflow Actions](#workflow-actions)
  - [Search](#search)
  - [Boot Data and Permissions](#boot-data-and-permissions)
  - [Frappe Convenience Helpers](#frappe-convenience-helpers)
- [Read-Only Guard](#read-only-guard)
- [Error Handling](#error-handling)
- [API Reference](#api-reference)
  - [RjssClient](#rjssclient)
  - [ClientConfig](#clientconfig)
  - [AuthMode](#authmode)
  - [SessionInfo](#sessioninfo)
  - [FrappeBoot](#frappeboot)
  - [ReportBuilder](#reportbuilder)
  - [ResourceBuilder](#resourcebuilder)
  - [WorkflowTransition](#workflowtransition)
  - [JssError](#jsserror)
  - [AuthEndpoints Trait](#authendpoints-trait)
- [Architecture](#architecture)
- [Development](#development)
  - [Mock Server](#mock-server)
  - [Testing](#testing)
  - [CI Pipeline](#ci-pipeline)
- [Security Considerations](#security-considerations)
- [License](#license)

---

## Features

- **Dual authentication** — cookie-based session login (email/password) or token-based API key/secret.
- **Automatic re-login** — session expiration detected and handled transparently with configurable retry limit.
- **CSRF token extraction** — parsed directly from `frappe.boot` on the `/app` page after login; no manual configuration.
- **Retry with exponential backoff** — configurable retries for transient errors (5xx, 429, network failures).
- **Typed builders** — ergonomic chained builders for reports and DocType queries.
- **Read-only safety guard** — prevents accidental POST/PUT/DELETE requests to non-whitelisted endpoints.
- **Rich error model** — full mirror of Frappe JSON errors with `exc_type`, `_server_messages`, and HTTP status.
- **Boot data accessors** — direct access to `frappe.boot` fields: user, roles, permissions, sidebar, workspaces.
- **File operations** — upload, download, download-to-path, and PDF print-format retrieval.
- **Workflow support** — query transitions and apply workflow actions with a single call.
- **Zero framework** — no macros, no runtime magic; plain `async` / `await`.
- **Trace IDs** — UUID v4 per client instance, included in every log line.
- **Pagination helper** — `.all()` walks all pages and returns a typed `Vec<T>`.
- **Concurrency-safe** — the client holds a persistent `reqwest::Client` with connection pooling and cookie jar.
- **Crypto-grade secret handling** — credentials wrapped in `secrecy::SecretString`, zeroized on drop.

---

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
librjss = { git = "https://github.com/mroczect/librjss", branch = "master" }
tokio = { version = "1", features = ["full"] }
secrecy = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

If the crate is published on crates.io:

```toml
[dependencies]
librjss = "2.3.0"
```

Minimum supported Rust version: **1.88** (edition 2024).

---

## Quick Start

```rust
use librjss::{AuthMode, ClientConfig, RjssClient};
use secrecy::SecretString;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ClientConfig {
        base_url: "https://erp.example.com".parse()?,
        auth_mode: AuthMode::Session {
            email: SecretString::new("user@example.com".into()),
            password: SecretString::new("hunter2".into()),
        },
        expected_sitename: Some("erp.example.com".into()),
        required_roles: vec!["System Manager".into()],
        timeout_secs: 30,
        max_retries: 3,
        user_agent: "my-app/1.0".into(),
        insecure_ssl: false,
        readonly_guard: true,
    };

    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    println!("logged in as {:?}", client.session_info().unwrap().full_name);

    let raw = client
        .doctype("Customer")
        .fields(vec!["name", "customer_name", "territory"])
        .filter("disabled", "=", "0")
        .order_by("creation desc")
        .limit(10)
        .execute_raw()
        .await?;

    println!("{}", raw);
    Ok(())
}
```

---

## Configuration

### Environment Variables

All `ClientConfig` fields can be populated from the environment via `ClientConfig::from_env()`:

| Variable                   | Default         | Description                                                     |
| -------------------------- | --------------- | --------------------------------------------------------------- |
| `JSS_BASE_URL` / `JSS_URL` | —               | **Required.** Base URL of the JSS instance (no trailing slash). |
| `JSS_EMAIL` / `JSS_USR`    | —               | Email for session authentication.                               |
| `JSS_PASSWORD` / `JSS_PWD` | —               | Password for session authentication.                            |
| `JSS_TOKEN_KEY`            | —               | API key for token auth. If set, token mode is used.             |
| `JSS_TOKEN_SECRET`         | —               | API secret for token auth. Required if `JSS_TOKEN_KEY` is set.  |
| `JSS_EXPECTED_SITENAME`    | —               | Expected site name; login fails on mismatch.                    |
| `JSS_REQUIRED_ROLES`       | —               | Comma-separated list of roles the user must have.               |
| `JSS_TIMEOUT_SECS`         | `30`            | Per-request timeout in seconds.                                 |
| `JSS_MAX_RETRIES`          | `3`             | Maximum retry attempts for transient failures.                  |
| `JSS_USER_AGENT`           | `librjss/2.3.0` | User-Agent header.                                              |
| `JSS_INSECURE_SSL`         | `false`         | Accept invalid TLS certificates.                                |
| `JSS_READONLY_GUARD`       | `true`          | Enable the read-only guard. Set `false` to disable.             |

Example `.env` file:

```bash
JSS_BASE_URL=https://erp.example.com
JSS_EMAIL=user@example.com
JSS_PASSWORD=hunter2
JSS_EXPECTED_SITENAME=erp.example.com
JSS_REQUIRED_ROLES=System Manager,Desk User
JSS_TIMEOUT_SECS=30
JSS_MAX_RETRIES=3
JSS_READONLY_GUARD=true
```

Load and construct the client:

```rust
let cfg = ClientConfig::from_env()?;
let mut client = RjssClient::new(cfg)?;
client.authenticate().await?;
```

### Programmatic Configuration

```rust
use librjss::{AuthMode, ClientConfig};
use secrecy::SecretString;

let cfg = ClientConfig {
    base_url: "https://erp.example.com".parse()?,
    auth_mode: AuthMode::Session {
        email: SecretString::new("user@example.com".into()),
        password: SecretString::new("hunter2".into()),
    },
    expected_sitename: Some("erp.example.com".into()),
    required_roles: vec!["Admin".into()],
    timeout_secs: 30,
    max_retries: 3,
    user_agent: "my-app/1.0".into(),
    insecure_ssl: false,
    readonly_guard: true,
};
```

Configuration is validated by `RjssClient::new`. Validation rules:

- HTTPS is enforced unless `insecure_ssl = true`.
- For `Session` mode, email and password must not be empty.
- URL scheme `data`, `javascript`, and `vbscript` are rejected.

---

## Authentication

### Session Auth

Cookie-based, equivalent to logging in via the web UI.

1. `POST /api/method/login` with `usr` and `pwd` in the form body.
2. Server responds with a session cookie and JSON.
3. Library fetches `/app`, parses `frappe.boot`, extracts the CSRF token, and stores the site name and user info.

```rust
let cfg = ClientConfig {
    auth_mode: AuthMode::Session {
        email: SecretString::new("user@example.com".into()),
        password: SecretString::new("hunter2".into()),
    },
    // ...
    ..Default::default()
};
```

Pros:

- Full parity with browser session.
- No need for admin-generated API keys.

Cons:

- Session can expire; handled automatically by `ensure_session`.
- Sensitive to server-side session policies (`deny_multiple_sessions`, etc.).

### Token Auth

Stateless. Every request includes `Authorization: token <api_key>:<api_secret>`.

```rust
let cfg = ClientConfig {
    auth_mode: AuthMode::Token {
        api_key: "abcd1234".into(),
        api_secret: SecretString::new("efgh5678".into()),
    },
    // ...
};
```

Pros:

- No login roundtrip.
- No session expiry.
- Ideal for CI jobs, cron tasks, and services.

Cons:

- Requires an administrator to generate keys.
- Every API key carries the full permissions of its owning user.

### Auto Re-login

In session mode, `ensure_session()`:

1. Calls `frappe.auth.get_logged_user`.
2. If the response is `401 Unauthorized`, drops the current session.
3. Attempts to re-login using cached credentials.
4. Repeats up to `MAX_REAUTH_ATTEMPTS` (3 by default).

You can invoke it explicitly:

```rust
client.ensure_session().await?;
```

Or let the library handle it implicitly on the next authenticated request.

---

## Usage Examples

### Basic HTTP Requests

All four HTTP methods are available as authenticated wrappers:

```rust
let body = client.authenticated_get("/api/resource/ToDo").await?;

let resp = client
    .authenticated_post(
        "/api/resource/ToDo",
        r#"{"description":"check the logs","status":"Open"}"#,
    )
    .await?;

let updated = client
    .authenticated_put(
        "/api/resource/ToDo/TODO-0001",
        r#"{"status":"Closed"}"#,
    )
    .await?;

let deleted = client
    .authenticated_delete("/api/resource/ToDo/TODO-0001")
    .await?;
```

Form-encoded POSTs (used by Frappe for search, workflow, and list views):

```rust
let result = client
    .post_form(
        "/api/method/frappe.client.get_count",
        &[("doctype", "Customer")],
    )
    .await?;
```

### Working with DocTypes

The `ResourceBuilder` provides a fluent interface for list queries.

```rust
let list: serde_json::Value = client
    .doctype("Sales Invoice")
    .filter("status", "=", "Unpaid")
    .filter("customer", "=", "ACME")
    .fields(vec!["name", "customer", "grand_total", "posting_date"])
    .order_by("posting_date desc")
    .limit(50)
    .limit_start(0)
    .execute()
    .await?;
```

Supported filter operators mirror Frappe:

| Operator          | Meaning                            |
| ----------------- | ---------------------------------- |
| `=`               | Equal                              |
| `!=`              | Not equal                          |
| `>` `<` `>=` `<=` | Comparison                         |
| `like`            | SQL LIKE (with `%`)                |
| `in`              | In list — pass a JSON array string |
| `not in`          | Not in list                        |
| `is`              | `set` or `not set`                 |
| `between`         | Between two values                 |

Example with `is`:

```rust
client.doctype("Customer").filter("email_id", "is", "set")
```

### Pagination

`.all()` walks pages automatically (page size 200) until the server returns fewer than the page size.

```rust
#[derive(Debug, serde::Deserialize)]
struct Customer {
    name: String,
    customer_name: String,
}

let all: Vec<Customer> = client
    .doctype("Customer")
    .filter("disabled", "=", "0")
    .fields(vec!["name", "customer_name"])
    .all()
    .await?;

println!("{} active customers", all.len());
```

Internally `all()` re-issues the same query with incrementing `limit_start` and stops when a page returns fewer rows than the requested size.

### Single Document CRUD

```rust
// Read
let doc = client.get_doc("Customer", "CUST-0001").await?;
let json: serde_json::Value = client.get_doc_json("Customer", "CUST-0001").await?;

// Create
#[derive(serde::Serialize)]
struct NewCustomer {
    customer_name: String,
    customer_group: String,
    territory: String,
}

let new_customer = NewCustomer {
    customer_name: "ACME Corporation".into(),
    customer_group: "Commercial".into(),
    territory: "Indonesia".into(),
};

let created = client.create_doc("Customer", &new_customer).await?;

// Update
#[derive(serde::Serialize)]
struct Update {
    website: String,
}

let updated = client
    .update_doc("Customer", "CUST-0001", &Update {
        website: "https://acme.example.com".into(),
    })
    .await?;

// Delete
client.delete_doc("Customer", "CUST-9999").await?;
```

> All write operations go through the guard. If the guard is on and the path isn't whitelisted, the request never leaves your process.

### Reports

```rust
#[derive(Debug, serde::Deserialize)]
struct Row {
    name: String,
    total: f64,
}

let rows: Vec<Row> = client
    .report("Sales Analytics")
    .add_filter("from_date", "2026-01-01")
    .add_filter("to_date", "2026-06-30")
    .run()
    .await?;
```

For raw JSON:

```rust
let raw = client
    .report("Sales Analytics")
    .add_filter("from_date", "2026-01-01")
    .run_raw()
    .await?;
```

> **Note.** Frappe Script Reports may have side effects (comments, ToDos, audit logs). The read-only guard blocks `query_report.run` by default. Add the path to the whitelist only if you've audited the report.

### Files and PDFs

**Upload:**

```rust
let bytes = std::fs::read("invoice.pdf")?;
let resp = client
    .upload_file(
        "invoice.pdf",
        bytes,
        "Sales Invoice",
        "SINV-0001",
        "attach",
    )
    .await?;
```

**Download:**

```rust
let bytes = client.download_file("/files/invoice.pdf").await?;
std::fs::write("local.pdf", bytes)?;

// Or write directly to a path
client
    .download_file_to_path(
        "/files/invoice.pdf",
        std::path::Path::new("local.pdf"),
    )
    .await?;
```

**PDF from print format:**

```rust
let pdf = client
    .download_pdf_kartu_piutang(
        "Master Data Nasabah",
        "ODR-00001",
        "Form Rincian Sisa Piutang Nasabah",
        false, // no_letterhead
    )
    .await?;
std::fs::write("piutang.pdf", pdf)?;
```

### Workflow Actions

Fetch available transitions:

```rust
let transitions = client
    .workflow_transitions("Sales Order", "SO-0001")
    .await?;

for t in &transitions {
    println!("{} → {} (action: {})",
        t.state,
        t.next_state.as_deref().unwrap_or("?"),
        t.action,
    );
}
```

Apply a workflow action:

```rust
client
    .apply_workflow("Sales Order", "SO-0001", "Approve")
    .await?;
```

Transition to a target state directly:

```rust
client
    .transition_to_state("Sales Order", "SO-0001", "Approved")
    .await?;
```

> Workflow writes are blocked by the guard unless `POST /api/method/frappe.model.workflow.apply_workflow` is whitelisted or the guard is disabled.

### Search

Global search (navbar-style):

```rust
let results = client.global_search("acme", 10, Some("Customer")).await?;
```

Link field autocomplete:

```rust
let results = client
    .search_link("ac", "Customer", "Sales Invoice", 10)
    .await?;
```

### Boot Data and Permissions

`frappe.boot` is parsed and cached after a successful login. All accessors are zero-cost lookups.

```rust
if let Some(boot) = client.boot() {
    println!("site: {}", boot.sitename);
    println!("user: {} <{}>", boot.user.full_name, boot.user.email);
    println!("roles: {:?}", boot.user.roles);
    println!("can_read: {} doctypes", boot.user.can_read.len());
    println!("can_write: {} doctypes", boot.user.can_write.len());
    println!("versions: {:?}", boot.versions);
}
```

Convenience methods on `RjssClient`:

```rust
if client.can_write("Sales Invoice") {
    // safe to attempt a write (guard permitting)
}

if client.can_read("Customer") {
    // user has read permission on Customer
}

let all = client.accessible_doctypes(); // union of can_read, can_write, can_create
```

Additional boot accessors:

```rust
client.user_info_map();
client.sidebar_pages();
client.navbar_settings();
client.versions();
client.lang_dict();
client.frequent_links();
client.is_developer_mode();
client.is_read_only();
```

### Frappe Convenience Helpers

Thin wrappers over commonly used Frappe methods:

```rust
// Count documents
let n = client
    .get_count("Customer", "[]", "[]", false)
    .await?;

// Report list with view settings
let list = client
    .get_list(
        "Customer",
        r#"["name","customer_name"]"#,
        r#"[["disabled","=","0"]]"#,
        "[]",
        "creation desc",
        0,
        50,
        "List",
        "",
        false,
    )
    .await?;

// Doctype metadata
let meta = client.get_doctype_meta("Customer").await?;

// Notifications and events
let notifs = client.get_notifications(20).await?;
let events = client.get_events("2026-01-01", "2026-01-31").await?;

// Desktop workspace
let page = client
    .get_desktop_page("Sales", "Sales", true)
    .await?;

// Call an arbitrary whitelisted method
use std::collections::HashMap;
let mut args = HashMap::new();
args.insert("doctype".into(), "Sales Invoice".into());
args.insert("docname".into(), "SINV-0001".into());
let resp = client.call_method("frappe.email.send", Some(args)).await?;
```

---

## Read-Only Guard

The read-only guard is a defensive layer that intercepts all outbound write requests (POST, PUT, DELETE) and blocks any path that isn't explicitly whitelisted.

**Why.** It prevents accidental mutations during development, testing, or when you're running analysis tools that shouldn't be allowed to write.

**How.** The `guard_check` method is called by every authenticated mutator before the request leaves your process. If the path isn't in the whitelist, an error is returned:

```
🛡️  READ-ONLY GUARD menolak POST /api/resource/Customer
    Alasan: path tidak ada di whitelist read-only.
    Kalau kamu memang mau mutasi, set `readonly_guard: false` di ClientConfig.
```

**Default whitelist.** Read-only Frappe method calls that use POST because of form encoding:

```
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
```

Application-specific read-only endpoints can be added by extending `ReadOnlyGuard::new()` in `src/client/guard.rs`.

**Toggle.** Disable globally:

```rust
ClientConfig { readonly_guard: false, ..cfg }
```

Or via environment: `JSS_READONLY_GUARD=false`.

**Extend.** Add paths to the whitelist by modifying `safe_post_paths` in the guard. Only add endpoints you've audited as read-only.

---

## Error Handling

`JssError` covers every failure mode. Match on the variant to handle specific cases:

```rust
use librjss::JssError;

match client.authenticated_get("/api/resource/ToDo").await {
    Ok(body) => println!("{}", body),

    Err(JssError::ApiError { exc_type, message, status }) => {
        eprintln!("[{status}] {exc_type}: {message}");
    }

    Err(JssError::RateLimited { retry_after }) => {
        let secs = retry_after.unwrap_or(5);
        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        // retry the request
    }

    Err(JssError::NotAuthenticated) => {
        // call client.ensure_session().await or re-login
    }

    Err(JssError::Permission(msg)) => {
        eprintln!("access denied: {msg}");
    }

    Err(JssError::Network(e)) => {
        eprintln!("network error: {e}");
    }

    Err(e) => {
        eprintln!("unhandled: {e}");
        let code = e.status_code();
        eprintln!("mapped HTTP status: {code}");
    }
}
```

`JssError::from_api_response(status, body)` parses Frappe's JSON error envelope:

```json
{
  "exc_type": "ValidationError",
  "_server_messages": "[\"{\\\"message\\\": \\\"Field X is required\\\"}\"]",
  "exc": "Traceback..."
}
```

The library extracts `exc_type` and either `_server_messages` or `exc` into a structured `ApiError`.

`JssError::status_code()` maps each variant to a logical HTTP status code (useful when wrapping the client in your own service).

---

## API Reference

### RjssClient

Central client struct. Holds config, HTTP connection pool, session, credentials, boot data, and the read-only guard.

#### Construction

```rust
pub fn new(config: ClientConfig) -> Result<Self, JssError>
```

Validates config, builds `reqwest::Client` with cookie jar, sets timeout, user-agent, TLS policy, and initializes the guard.

#### Authentication and Session

| Method                      | Returns                | Description                                        |
| --------------------------- | ---------------------- | -------------------------------------------------- |
| `authenticate(&mut self)`   | `Result<(), JssError>` | Logs in using the configured auth mode.            |
| `logout(&mut self)`         | `Result<(), JssError>` | Session-mode only. Clears session and credentials. |
| `ensure_session(&mut self)` | `Result<(), JssError>` | Re-validates the session; re-logs in if expired.   |

#### Accessors

| Method                   | Returns                |
| ------------------------ | ---------------------- |
| `base_url()`             | `&Url`                 |
| `trace_id()`             | `&str`                 |
| `session_info()`         | `Option<&SessionInfo>` |
| `boot()`                 | `Option<&FrappeBoot>`  |
| `is_readonly_guard_on()` | `bool`                 |

#### Permission Checks

| Method                  | Description                                     |
| ----------------------- | ----------------------------------------------- |
| `can_read(doctype)`     | User can read documents of this DocType.        |
| `can_write(doctype)`    | User can modify documents.                      |
| `can_create(doctype)`   | User can create new documents.                  |
| `can_submit(doctype)`   | User can submit documents.                      |
| `can_delete(doctype)`   | User can delete documents.                      |
| `accessible_doctypes()` | Union of `can_read`, `can_write`, `can_create`. |

All return `bool` and read from cached boot data — no network request.

#### HTTP Request Methods

| Method                 | Signature                                                                |
| ---------------------- | ------------------------------------------------------------------------ |
| `authenticated_get`    | `(&self, path: &str) -> Result<String, JssError>`                        |
| `authenticated_post`   | `(&self, path: &str, body_json: &str) -> Result<String, JssError>`       |
| `authenticated_put`    | `(&self, path: &str, body_json: &str) -> Result<String, JssError>`       |
| `authenticated_delete` | `(&self, path: &str) -> Result<String, JssError>`                        |
| `post_form`            | `(&self, path: &str, form: &[(&str, &str)]) -> Result<String, JssError>` |

Each attaches the correct authorization header, includes CSRF when needed, and retries transient errors with exponential backoff.

#### File Operations

```rust
pub async fn upload_file(
    &self,
    file_name: &str,
    file_content: Vec<u8>,
    doctype: &str,
    docname: &str,
    fieldname: &str,
) -> Result<String, JssError>

pub async fn download_file(&self, file_url: &str) -> Result<Vec<u8>, JssError>

pub async fn download_file_to_path(
    &self,
    file_url: &str,
    save_path: &Path,
) -> Result<(), JssError>

pub async fn download_pdf_kartu_piutang(
    &self,
    doctype: &str,
    name: &str,
    format: &str,
    no_letterhead: bool,
) -> Result<Vec<u8>, JssError>
```

#### Builders and Shortcuts

```rust
pub fn report(&self, name: &str) -> ReportBuilder<'_>
pub fn doctype(&self, name: &str) -> ResourceBuilder<'_>

pub async fn get_doc(&self, doctype: &str, name: &str) -> Result<String, JssError>
pub async fn get_doc_json(&self, doctype: &str, name: &str) -> Result<serde_json::Value, JssError>

pub async fn create_doc(&self, doctype: &str, data: &impl Serialize) -> Result<String, JssError>
pub async fn update_doc(&self, doctype: &str, name: &str, data: &impl Serialize) -> Result<String, JssError>
pub async fn delete_doc(&self, doctype: &str, name: &str) -> Result<String, JssError>
```

#### Frappe Convenience

```rust
pub async fn global_search(&self, query: &str, limit: u32, doctype: Option<&str>) -> Result<String, JssError>
pub async fn search_link(&self, txt: &str, doctype: &str, reference_doctype: &str, page_length: u32) -> Result<String, JssError>
pub async fn get_transitions(&self, doc_json: &str) -> Result<String, JssError>
pub async fn save_user_settings(&self, doctype: &str, user_settings: &str) -> Result<String, JssError>
pub async fn get_count(&self, doctype: &str, filters_json: &str, fields_json: &str, distinct: bool) -> Result<String, JssError>
pub async fn get_list_settings(&self, doctype: &str) -> Result<String, JssError>
pub async fn get_doctype_meta(&self, doctype: &str) -> Result<String, JssError>
pub async fn get_notifications(&self, limit: u32) -> Result<String, JssError>
pub async fn get_events(&self, start: &str, end: &str) -> Result<String, JssError>
pub async fn get_desktop_page(&self, name: &str, title: &str, public: bool) -> Result<String, JssError>
pub async fn get_lazy_child_rows(&self, docname: &str, tab: &str) -> Result<String, JssError>
pub async fn run_report(&self, report_name: &str, filters: serde_json::Value) -> Result<String, JssError>
pub async fn call_method(&self, method: &str, args: Option<HashMap<String, String>>) -> Result<String, JssError>
```

### ClientConfig

```rust
pub struct ClientConfig {
    pub base_url: reqwest::Url,
    pub auth_mode: AuthMode,
    pub expected_sitename: Option<String>,
    pub required_roles: Vec<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub user_agent: String,
    pub insecure_ssl: bool,
    pub readonly_guard: bool,
}
```

| Field               | Purpose                                                               |
| ------------------- | --------------------------------------------------------------------- |
| `base_url`          | Server URL. Must include scheme and host. No trailing slash required. |
| `auth_mode`         | `Session` or `Token`. See below.                                      |
| `expected_sitename` | If set, login fails unless the boot sitename matches.                 |
| `required_roles`    | If non-empty, login fails unless the user has at least one.           |
| `timeout_secs`      | Per-request timeout.                                                  |
| `max_retries`       | Retry attempts for transient failures.                                |
| `user_agent`        | Sent with every request.                                              |
| `insecure_ssl`      | Accept self-signed / invalid TLS certificates.                        |
| `readonly_guard`    | Enable the read-only guard.                                           |

Validated on `RjssClient::new`.

### AuthMode

```rust
pub enum AuthMode {
    Session {
        email: SecretString,
        password: SecretString,
    },
    Token {
        api_key: String,
        api_secret: SecretString,
    },
}
```

Secrets are wrapped in `SecretString` — never serialized, zeroized on drop.

### SessionInfo

```rust
pub struct SessionInfo {
    pub sid: SecretString,
    pub csrf_token: SecretString,
    pub full_name: Option<String>,
    pub sitename: String,
    pub roles: Vec<String>,
}
```

Available after successful `authenticate()`.

### FrappeBoot

Parsed from the `frappe.boot` JavaScript object on the `/app` page. All fields have sensible defaults so future additions won't break deserialization.

Selected fields:

```rust
pub struct FrappeBoot {
    pub user: BootUser,
    pub sitename: String,
    pub csrf_token: String,
    pub sysdefaults: SysDefaults,
    pub app_logo_url: Option<String>,
    pub home_page: Option<String>,
    pub allow_modules: Vec<String>,
    pub can_select: Vec<String>,
    pub can_create: Vec<String>,
    pub can_write: Vec<String>,
    pub can_read: Vec<String>,
    pub can_submit: Vec<String>,
    pub can_cancel: Vec<String>,
    pub can_delete: Vec<String>,
    pub can_get_report: Vec<String>,
    pub all_reports: HashMap<String, ReportMeta>,
    pub module_wise_workspaces: HashMap<String, Vec<String>>,
    pub dashboards: Vec<DashboardMeta>,
    pub single_types: Vec<String>,
    pub calendars: Vec<String>,
    pub treeviews: Vec<String>,
    pub module_app: HashMap<String, String>,
    pub app_data: Vec<AppData>,
    pub user_info: HashMap<String, BootUserInfo>,
    pub sidebar_pages: SidebarPages,
    pub navbar_settings: Option<NavbarSettings>,
    pub versions: HashMap<String, String>,
    pub lang_dict: HashMap<String, String>,
    pub lang: Option<String>,
    pub page_info: HashMap<String, PageInfo>,
    pub frequently_visited_links: Vec<FrequentLink>,
    pub developer_mode: i32,
    pub read_only: bool,
    pub socketio_port: Option<u16>,
    pub desk_settings: HashMap<String, serde_json::Value>,
    pub desk_theme: Option<String>,
}
```

Supporting types:

- `BootUser` — `name`, `email`, `full_name`, `roles`, `user_type`, `permissions`, all `can_*` vectors.
- `SysDefaults` — `default_app`, `time_zone`.
- `ReportMeta` — `modified`, `title`, `ref_doctype`, `report_type`.
- `AppData` — `app_name`, `app_title`, `app_route`, `app_logo_url`, `modules`, `workspaces`.
- `BootUserInfo` — `fullname`, `image`, `name`, `email`, `time_zone`.
- `SidebarPages` — `workspace_setup_completed`, `pages`, `has_access`, `has_create_access`.
- `NavbarSettings` — `settings_dropdown`, `help_dropdown` (each a `Vec<NavbarItem>`).
- `FrequentLink` — `route`, `count`.

### ReportBuilder

```rust
pub struct ReportBuilder<'a> { /* ... */ }

impl<'a> ReportBuilder<'a> {
    pub fn add_filter(self, field: &str, value: &str) -> Self;

    pub async fn run_raw(&self) -> Result<String, JssError>;
    pub async fn run<T: DeserializeOwned>(&self) -> Result<T, JssError>;
}
```

Created via `client.report("Report Name")`. Filters are key-value strings, matching Frappe's query report filter format.

### ResourceBuilder

```rust
pub struct ResourceBuilder<'a> { /* ... */ }

impl<'a> ResourceBuilder<'a> {
    pub fn filter(self, field: &str, operator: &str, value: &str) -> Self;
    pub fn fields(self, fields: Vec<&str>) -> Self;
    pub fn order_by(self, order: &str) -> Self;
    pub fn limit(self, limit: u32) -> Self;
    pub fn limit_start(self, start: u32) -> Self;

    pub async fn execute_raw(&self) -> Result<String, JssError>;
    pub async fn execute<T: DeserializeOwned>(&self) -> Result<T, JssError>;
    pub async fn all<T: DeserializeOwned>(&self) -> Result<Vec<T>, JssError>;
}
```

Created via `client.doctype("DocType Name")`.

`all()` paginates with a page size of 200 and returns `Vec<T>`.

### WorkflowTransition

```rust
pub struct WorkflowTransition {
    pub action: String,
    pub state: String,
    pub next_state: Option<String>,
    pub allowed: Option<String>,
    pub allow_self_approval: Option<i32>,
}
```

Returned by `workflow_transitions()`.

### JssError

```rust
pub enum JssError {
    Config(String),
    Validation(String),
    Network(reqwest::Error),
    Http { status: StatusCode, body: String },
    ApiError {
        exc_type: String,
        message: String,
        status: StatusCode,
    },
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
```

Constructors and helpers:

- `JssError::from_api_response(status, body)` — parses Frappe JSON error envelope.
- `JssError::status_code()` — maps each variant to a logical HTTP status code.

### AuthEndpoints Trait

Static URL builders, usable without a client instance.

```rust
pub trait AuthEndpoints {
    fn login_url(base: &Url) -> Result<Url, JssError>;
    fn logout_url(base: &Url) -> Result<Url, JssError>;
    fn csrf_token_url(base: &Url) -> Result<Url, JssError>;
    fn get_logged_user_url(base: &Url) -> Result<Url, JssError>;
    fn app_page_url(base: &Url) -> Result<Url, JssError>;
}
```

Implemented by `RjssClient`. Useful for unit tests and URL validation.

---

## Architecture

```
librjss/
├── src/
│   ├── lib.rs                     # Public API re-exports
│   ├── api/
│   │   └── auth/
│   │       └── endpoints.rs       # AuthEndpoints trait (URL builders)
│   ├── bin/
│   │   └── verify_readonly.rs     # Snapshot/comparison binary
│   ├── client/
│   │   ├── mod.rs                 # RjssClient
│   │   ├── guard.rs               # ReadOnlyGuard
│   │   ├── file.rs                # File operations
│   │   ├── report.rs              # ReportBuilder
│   │   ├── resource.rs            # ResourceBuilder + CRUD helpers
│   │   ├── auth/
│   │   │   ├── login.rs           # Login flow
│   │   │   ├── logout.rs          # Logout flow
│   │   │   ├── session.rs         # ensure_session / auto re-login
│   │   │   ├── token.rs           # Token-mode synthetic session
│   │   │   ├── app_parser.rs      # Parse frappe.boot from /app HTML
│   │   │   └── http_helpers.rs    # Shared HTTP logic (backoff, join, classify)
│   │   └── methods/
│   │       ├── get.rs
│   │       ├── post.rs
│   │       ├── put.rs
│   │       ├── delete.rs
│   │       ├── post_form.rs
│   │       └── workflow.rs        # Workflow transitions & apply
│   └── handler/
│       ├── config/                # ClientConfig, AuthMode, env, validation
│       ├── error.rs               # JssError
│       └── types/
│           ├── boot.rs            # FrappeBoot and nested types
│           ├── login.rs
│           ├── session.rs
│           └── user_info.rs
└── tests/                         # Integration tests
```

**Design principles:**

1. **Immutability of the request pipeline.** Every HTTP call goes through the same `http_helpers` primitives for auth injection, URL joining, backoff, and response classification.
2. **Explicit error handling.** No panics, no `.unwrap()` in library code, no silent failures.
3. **Secrets never leak.** Credentials and tokens live in `SecretString` and are only exposed at the moment of building a request header.
4. **Guard-first design.** All mutating methods call `guard_check` before touching the network.
5. **Type-safe boot parsing.** All boot fields are `Option` or have defaults so Frappe upgrades won't break the crate.

---

## Development

### Mock Server

A full mock server for offline development is available at `examples/jss_mock_server.rs`. It emulates:

- `POST /api/method/login` — sets a cookie and returns a JSON envelope.
- `GET /app` — returns HTML with a synthetic `frappe.boot`.
- `GET /api/method/frappe.auth.get_logged_user`
- `POST /api/method/logout`
- `GET /api/method/frappe.auth.get_csrf_token`

Start it:

```bash
cargo run --example jss_mock_server
```

Then point your client to `http://127.0.0.1:8080` with `JSS_INSECURE_SSL=true` (self-signed).

### Testing

Unit tests (no server):

```bash
cargo test --lib
```

Integration tests require the mock server running:

```bash
cargo test --test client_tests
cargo test --test file_tests
cargo test --test report_tests
cargo test --test resource_tests
cargo test --test api_auth_tests
cargo test --test env_tests
cargo test --test error_tests
cargo test --test types_tests
```

Or run everything:

```bash
cargo test --workspace
```

### CI Pipeline

`.github/workflows/ci.yml` runs on every push and pull request to `master`:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo check --workspace`
4. `cargo test --lib`
5. Start mock server → run integration tests
6. `cargo build --release`
7. Upload `cli_auth` artifact

To reproduce locally:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

---

## Security Considerations

- **Never hardcode credentials.** Use environment variables or a secret manager. The `ClientConfig::from_env()` helper is designed for this.
- **Never commit `.env` files.** Add `.env` to `.gitignore`.
- **Prefer API keys with minimal roles.** A token inherits all permissions of its user. Create dedicated service users with the least privileges required.
- **Watch the read-only guard.** Leave it on unless you have a specific reason to disable it. It's a cheap safety net.
- **Rotate credentials.** If a password or API key leaks, rotate immediately. Sessions can be invalidated from the UI (`User → Logout from all sessions`).
- **HTTPS only.** `insecure_ssl = true` should only be used against local development servers with self-signed certificates.
- **Logs.** The library logs request metadata (path, status, body hash) at `debug` level. It does **not** log request bodies, headers, or secrets. Trace IDs let you correlate requests across services without exposing sensitive data.

---

## License

MIT License. See [LICENSE](LICENSE).

Copyright (c) 2026 mroczect &lt;mroczect@proton.me&gt;

---

## See Also

- [Frappe Framework Documentation](https://frappeframework.com/docs)
- [ERPNext Documentation](https://docs.erpnext.com)
- [reqwest](https://docs.rs/reqwest)
- [serde](https://docs.rs/serde)
- [secrecy](https://docs.rs/secrecy)
