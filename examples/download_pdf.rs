use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::cookie::Jar;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const DOCTYPE: &str = "Surat Peringatan KSP";
const DEFAULT_DOC_NAMES: &[&str] = &["JD4546", "JD4547", "JD4548", "JD4549", "JD4550"];

const FORCE_PRINT_FORMAT: Option<&str> = None;

const FORCE_LETTERHEAD: Option<&str> = None;

const NO_LETTERHEAD: bool = false;
const OUTPUT_DIR: &str = "./pdf_output";
const DELAY_MS: u64 = 800;
const TIMEOUT_SECS: u64 = 60;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut list_formats_only = false;
    let mut forced_format: Option<String> = FORCE_PRINT_FORMAT.map(String::from);

    if args.iter().any(|a| a == "--list-formats") {
        list_formats_only = true;
        args.retain(|a| a != "--list-formats");
    }

    if let Some(idx) = args.iter().position(|a| a == "--format") {
        if idx + 1 < args.len() {
            forced_format = Some(args[idx + 1].clone());
            args.drain(idx..=idx + 1);
        } else {
            anyhow::bail!("--format butuh argumen nama print format");
        }
    }

    let doc_names = resolve_doc_names(&args)?;

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Frappe PDF Batch Downloader                             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("  DocType      : {}", DOCTYPE);
    println!("  Output       : {}", OUTPUT_DIR);

    let base_url = std::env::var("JSS_BASE_URL")
        .context("Env JSS_BASE_URL belum di-set")?
        .trim_end_matches('/')
        .to_string();
    let email = std::env::var("JSS_EMAIL").context("Env JSS_EMAIL belum di-set")?;
    let password = std::env::var("JSS_PASSWORD").context("Env JSS_PASSWORD belum di-set")?;

    let jar = Arc::new(Jar::default());
    let http = Client::builder()
        .cookie_provider(Arc::clone(&jar))
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .user_agent("librjss-pdf-downloader/1.0")
        .build()?;

    print!("🔐 Login sebagai {} ... ", email);
    use std::io::Write;
    std::io::stdout().flush()?;

    let resp = http
        .post(format!("{}/api/method/login", base_url))
        .form(&[("usr", &email), ("pwd", &password)])
        .send()
        .await?;

    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        println!("❌");
        anyhow::bail!("Login gagal (HTTP {}): {}", s, b);
    }
    println!("✅");

    let app_html = http
        .get(format!("{}/app", base_url))
        .send()
        .await?
        .text()
        .await?;
    let csrf_token = extract_csrf(&app_html).unwrap_or_default();
    println!(
        "🔎 CSRF token: {}",
        if csrf_token.is_empty() {
            "(kosong)".to_string()
        } else {
            format!("{} chars", csrf_token.len())
        }
    );

    println!("\n🔍 Mencari print format untuk doctype '{}' ...", DOCTYPE);
    let available_formats = list_print_formats(&http, &base_url, DOCTYPE).await?;

    if available_formats.is_empty() {
        println!("   ⚠️  Tidak ada print format custom, pakai 'Standard'");
    } else {
        println!("   Format tersedia ({}):", available_formats.len());
        for f in &available_formats {
            println!("     • {}", f);
        }
    }

    if list_formats_only {
        println!("\n✅ Selesai (mode --list-formats).");
        return Ok(());
    }

    let chosen_format: Option<String> =
        forced_format.or_else(|| available_formats.first().cloned());

    match &chosen_format {
        Some(f) => println!("\n✅ Pakai format: {}", f),
        None => println!("\n✅ Pakai format default (Standard)"),
    }

    let letterhead = FORCE_LETTERHEAD.map(String::from);
    if let Some(lh) = &letterhead {
        println!("✅ Pakai letterhead: {}", lh);
    }

    std::fs::create_dir_all(OUTPUT_DIR)
        .with_context(|| format!("Gagal buat folder {}", OUTPUT_DIR))?;

    if doc_names.is_empty() {
        anyhow::bail!("Tidak ada dokumen untuk didownload.");
    }

    println!("\n📥 Mulai download {} dokumen...\n", doc_names.len());
    let mut success = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for (i, name) in doc_names.iter().enumerate() {
        print!("[{:>3}/{}] {:<20} ... ", i + 1, doc_names.len(), name);
        std::io::stdout().flush()?;

        match download_one(
            &http,
            &base_url,
            name,
            &csrf_token,
            chosen_format.as_deref(),
            letterhead.as_deref(),
        )
        .await
        {
            Ok(bytes) => {
                let filename = format!("{}.pdf", sanitize_filename(name));
                let save_path = PathBuf::from(OUTPUT_DIR).join(&filename);
                std::fs::write(&save_path, &bytes)
                    .with_context(|| format!("Gagal tulis {}", save_path.display()))?;
                println!("✅ {:>6} KB", bytes.len() / 1024);
                success += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                failed.push((name.to_string(), e.to_string()));
            }
        }

        if i < doc_names.len() - 1 {
            tokio::time::sleep(Duration::from_millis(DELAY_MS)).await;
        }
    }

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  RINGKASAN                                               ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("  Total  : {}", doc_names.len());
    println!("  Sukses : {}", success);
    println!("  Gagal  : {}", failed.len());
    if !failed.is_empty() {
        println!("\n  Detail gagal:");
        for (name, err) in &failed {
            println!("    • {} → {}", name, err);
        }
    }
    println!("  Output : {}", OUTPUT_DIR);
    println!("══════════════════════════════════════════════════════════\n");

    if !failed.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

async fn list_print_formats(http: &Client, base_url: &str, doctype: &str) -> Result<Vec<String>> {
    let filters = serde_json::json!([["doc_type", "=", doctype]]);
    let fields = serde_json::json!(["name", "standard", "disabled"]);

    let url = format!(
        "{}/api/method/frappe.client.get_list?doctype={}&filters={}&fields={}&limit_page_length=0&order_by=standard asc",
        base_url,
        urlencoding::encode("Print Format"),
        urlencoding::encode(&filters.to_string()),
        urlencoding::encode(&fields.to_string()),
    );

    let resp = http.get(&url).send().await?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        anyhow::bail!("get_list Print Format gagal (HTTP {}): {}", s, b);
    }

    let json: serde_json::Value = resp.json().await?;
    let formats: Vec<String> = json["message"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|v| v["disabled"].as_i64().unwrap_or(0) == 0)
                .filter_map(|v| v["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(formats)
}

fn resolve_doc_names(args: &[String]) -> Result<Vec<String>> {
    if args.is_empty() {
        return Ok(DEFAULT_DOC_NAMES.iter().map(|s| s.to_string()).collect());
    }
    if args.len() == 1 && args[0].contains("..") {
        return parse_range(&args[0]);
    }
    Ok(args.to_vec())
}

fn parse_range(spec: &str) -> Result<Vec<String>> {
    let (start_str, end_str) = spec
        .split_once("..")
        .with_context(|| format!("Format range salah: '{}'. Contoh: JD4546..JD4550", spec))?;

    let start_prefix: String = start_str
        .chars()
        .take_while(|c| !c.is_ascii_digit())
        .collect();
    let end_prefix: String = end_str
        .chars()
        .take_while(|c| !c.is_ascii_digit())
        .collect();

    if start_prefix != end_prefix {
        anyhow::bail!(
            "Prefix range tidak sama: '{}' vs '{}'",
            start_prefix,
            end_prefix
        );
    }

    let start_num: u32 = start_str[start_prefix.len()..]
        .parse()
        .with_context(|| format!("Angka awal tidak valid: {}", start_str))?;
    let end_num: u32 = end_str[end_prefix.len()..]
        .parse()
        .with_context(|| format!("Angka akhir tidak valid: {}", end_str))?;

    if start_num > end_num {
        anyhow::bail!("Range terbalik: {} > {}", start_num, end_num);
    }

    let count = (end_num - start_num + 1) as usize;
    if count > 1000 {
        anyhow::bail!("Range terlalu besar ({} dokumen). Maksimal 1000.", count);
    }

    Ok((start_num..=end_num)
        .map(|n| format!("{}{}", start_prefix, n))
        .collect())
}

async fn download_one(
    http: &Client,
    base_url: &str,
    name: &str,
    csrf_token: &str,
    format: Option<&str>,
    letterhead: Option<&str>,
) -> Result<Vec<u8>> {
    let mut url = format!(
        "{}/api/method/frappe.utils.print_format.download_pdf?doctype={}&name={}&no_letterhead={}",
        base_url,
        urlencoding::encode(DOCTYPE),
        urlencoding::encode(name),
        if NO_LETTERHEAD { "1" } else { "0" },
    );
    if let Some(f) = format {
        url.push_str(&format!("&format={}", urlencoding::encode(f)));
    }
    if let Some(lh) = letterhead {
        url.push_str(&format!("&letterhead={}", urlencoding::encode(lh)));
    }

    let mut req = http.get(&url);
    if !csrf_token.is_empty() {
        req = req.header("X-Frappe-CSRF-Token", csrf_token);
    }

    let resp = req.send().await?;
    let status = resp.status();

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(300).collect();
        anyhow::bail!("HTTP {} — {}", status, snippet.trim());
    }

    let bytes = resp.bytes().await?;
    if bytes.len() < 5 || &bytes[0..5] != b"%PDF-" {
        let prefix: String =
            String::from_utf8_lossy(&bytes[..bytes.len().min(80)]).replace('\n', " ");
        anyhow::bail!(
            "Response bukan PDF ({} bytes). Prefix: {}",
            bytes.len(),
            prefix
        );
    }

    Ok(bytes.to_vec())
}

fn extract_csrf(html: &str) -> Option<String> {
    let marker = "frappe.csrf_token";
    let pos = html.find(marker)?;
    let after = &html[pos + marker.len()..];
    let eq_pos = after.find('"')?;
    let start = eq_pos + 1;
    let end = after[start..].find('"')?;
    Some(after[start..start + end].to_string())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c => c,
        })
        .collect()
}
