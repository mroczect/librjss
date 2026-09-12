use librjss::{ClientConfig, JssError, RjssClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage:");
        eprintln!("  url_fetch <url>");
        eprintln!("  url_fetch <doctype> <name>");
        eprintln!();
        eprintln!("contoh:");
        eprintln!(
            "  url_fetch https://app.juragansejati.biz.id/app/master-data-nasabah/OD-26-6003-0000974-0001"
        );
        eprintln!("  url_fetch \"Master Data Nasabah\" OD-26-6003-0000974-0001");
        std::process::exit(1);
    }

    let cfg = ClientConfig::from_env()?;
    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    let (doctype, docname) = if args.len() == 1 {
        let (slug, docname) = parse_url(&args[0])?;
        let boot = client
            .boot()
            .ok_or_else(|| JssError::Config("boot data kosong".into()))?;
        let known = boot.user.can_read.clone();
        let doctype = resolve_doctype(&slug, &known).ok_or_else(|| {
            JssError::Config(format!(
                "doctype untuk slug '{}' tidak ditemukan di daftar akses",
                slug
            ))
        })?;
        (doctype, docname)
    } else {
        (args[0].clone(), args[1].clone())
    };

    eprintln!("doctype : {}", doctype);
    eprintln!("docname : {}", docname);
    eprintln!();

    let path = format!(
        "/api/method/frappe.desk.form.load.getdoc?doctype={}&name={}",
        urlencoding::encode(&doctype),
        urlencoding::encode(&docname),
    );

    let raw = client.authenticated_get(&path).await?;

    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(e.to_string()))?;

    println!("{}", serde_json::to_string_pretty(&v)?);

    Ok(())
}

fn parse_url(url: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let s = url.trim_start_matches("view-source:");

    let idx = s.find("/app/").ok_or("url tidak mengandung /app/")?;
    let rest = &s[idx + 5..];

    let rest = rest.split('?').next().unwrap_or(rest);
    let rest = rest.split('#').next().unwrap_or(rest);

    let parts: Vec<&str> = rest.split('/').filter(|x| !x.is_empty()).collect();
    if parts.len() < 2 {
        return Err("url harus /app/<doctype-slug>/<docname>".into());
    }

    let slug = parts[0].to_string();
    let docname = parts[1..].join("/");
    Ok((slug, docname))
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn resolve_doctype(slug: &str, known: &[String]) -> Option<String> {
    let target = slug.to_lowercase();
    known.iter().find(|dt| slugify(dt) == target).cloned()
}
