use librjss::{ClientConfig, JssError, RjssClient};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

struct Style {
    on: bool,
}

impl Style {
    fn new() -> Self {
        let on = std::env::var("NO_COLOR").is_err()
            && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
        Self { on }
    }
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{}m{}\x1b[0m", code, s)
        } else {
            s.to_string()
        }
    }
    fn bold(&self, s: &str) -> String {
        self.wrap("1", s)
    }
    fn dim(&self, s: &str) -> String {
        self.wrap("2", s)
    }
    fn green(&self, s: &str) -> String {
        self.wrap("32", s)
    }
    fn red(&self, s: &str) -> String {
        self.wrap("31", s)
    }
    fn yellow(&self, s: &str) -> String {
        self.wrap("33", s)
    }
    fn cyan(&self, s: &str) -> String {
        self.wrap("36", s)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CekTunggakanResponse {
    message: RincianHutang,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RincianHutang {
    #[serde(default)]
    csp_name: Option<String>,
    #[serde(default)]
    saldo_tabungan: f64,
    #[serde(default)]
    sisa_admin: f64,
    #[serde(default)]
    sisa_bunga: f64,
    #[serde(default)]
    sisa_pokok: f64,
    #[serde(default)]
    sisa_pokok_akhir: f64,
    #[serde(default)]
    total_denda: f64,
    #[serde(default)]
    total_hutang: f64,
    #[serde(default)]
    total_sisa_angsuran: f64,
    #[serde(default)]
    tabel_angsuran: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct FieldDef {
    fieldname: String,
    label: Option<String>,
    fieldtype: String,
    reqd: i32,
    options: Option<String>,
    hidden: i32,
    read_only: i32,
    depends_on: Option<String>,
}

#[derive(Debug, Serialize)]
struct Scenario {
    jenis: String,
    pengurangan_bunga: f64,
    pengurangan_denda: f64,
    total_pengurangan: f64,
    sisa_bunga_akhir: f64,
    sisa_denda_akhir: f64,
    total_bayar_nasabah: f64,
    hemat: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    if raw_args.is_empty() || raw_args[0] == "-h" || raw_args[0] == "--help" {
        print_help();
        return Ok(());
    }

    let style = Style::new();
    let json_mode = has_flag(&raw_args, "--json");

    let args: Vec<String> = raw_args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();

    let cfg = ClientConfig::from_env()?;
    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    if !client.is_readonly_guard_on() {
        eprintln!("guard tidak aktif, keluar");
        std::process::exit(2);
    }

    match args.first().map(String::as_str) {
        Some("schema") | Some("s") => {
            let meta = fetch_meta(&client).await?;
            print_schema(&style, &meta, json_mode)?;
        }
        Some("meta") | Some("m") => {
            let meta = fetch_meta(&client).await?;
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&meta)?);
            } else {
                print_meta(&style, &meta);
            }
        }
        Some("cek") | Some("c") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let mdn = fetch_mdn(&client, np).await?;
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&mdn)?);
            } else {
                print_mdn(&style, np, &mdn);
            }
        }
        Some("hutang") | Some("h") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let (mdn, rincian) = fetch_all(&client, np).await?;
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&rincian)?);
            } else {
                print_hutang_report(&style, np, &mdn, &rincian);
            }
        }
        Some("simulasi") | Some("sim") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let jenis = args.get(2).ok_or("butuh jenis (bunga|denda|both)")?;
            let nilai: f64 = args.get(3).ok_or("butuh nilai")?.parse()?;
            let alasan = args.get(4).map(String::as_str);

            let (mdn, rincian) = fetch_all(&client, np).await?;
            let sc = compute_scenario(jenis, nilai, &rincian)?;
            let payload = build_payload(np, &mdn, &rincian, &sc, alasan)?;

            if json_mode {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_simulasi(&style, np, &mdn, &rincian, &sc, &payload);
            }
        }
        Some("scenario") | Some("sc") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let nilai: f64 = args.get(2).ok_or("butuh nilai")?.parse()?;
            let (mdn, rincian) = fetch_all(&client, np).await?;
            print_scenario(&style, np, &mdn, &rincian, nilai);
        }
        Some("history") | Some("hist") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let limit: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
            let items = fetch_history(&client, np, limit).await?;
            print_history(&style, np, &items, json_mode)?;
        }
        Some("batch") | Some("b") => {
            let file = args.get(1).ok_or("butuh file daftar no_perjanjian")?;
            let jenis = args.get(2).map(String::as_str).unwrap_or("bunga");
            let nilai: f64 = args
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(100_000.0);
            let csv_mode = has_flag(&raw_args, "--csv");
            run_batch(&client, &style, file, jenis, nilai, csv_mode).await?;
        }
        Some("export") | Some("e") => {
            let np = args.get(1).ok_or("butuh no_perjanjian")?;
            let jenis = args.get(2).ok_or("butuh jenis")?;
            let nilai: f64 = args.get(3).ok_or("butuh nilai")?.parse()?;
            let out_dir = PathBuf::from(args.get(4).map(String::as_str).unwrap_or("."));
            let alasan = args.get(5).map(String::as_str);

            std::fs::create_dir_all(&out_dir)?;
            let (mdn, rincian) = fetch_all(&client, np).await?;
            let sc = compute_scenario(jenis, nilai, &rincian)?;
            let payload = build_payload(np, &mdn, &rincian, &sc, alasan)?;

            let safe = np.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let path = out_dir.join(format!("deviasi-{}-{}.json", safe, ts));

            std::fs::write(&path, serde_json::to_string_pretty(&payload)?)?;
            println!(
                "  {} {} ({} bytes)",
                style.green("✓"),
                path.display(),
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            );
        }
        Some("validate") | Some("v") => {
            let file = args.get(1);
            let payload = match file {
                Some(f) => std::fs::read_to_string(f)?,
                None => {
                    eprintln!("baca payload dari stdin (Ctrl+D selesai)...");
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                }
            };
            let meta = fetch_meta(&client).await?;
            let v: serde_json::Value = serde_json::from_str(&payload)?;
            validate_payload(&style, &meta, &v);
        }
        Some("explore") => {
            let meta = fetch_meta(&client).await?;
            print_schema(&style, &meta, false)?;
            println!();

            println!("{}", style.bold("Deviasi terbaru (5):"));
            let items = fetch_recent(&client, 5).await?;
            for it in &items {
                println!(
                    "  {} | {} | {} | {} | {}",
                    it["name"].as_str().unwrap_or("-"),
                    trunc(it["no_perjanjian"].as_str().unwrap_or("-"), 28),
                    it["jenis_deviasi"].as_str().unwrap_or("-"),
                    it["status"].as_str().unwrap_or("-"),
                    rupiah(it["total_pengurangan"].as_f64().unwrap_or(0.0)),
                );
            }
        }
        Some("diff") | Some("d") => {
            let a = args.get(1).ok_or("butuh dev1")?;
            let b = args.get(2).ok_or("butuh dev2")?;
            let da = fetch_doc(&client, "Deviasi", a).await?;
            let db = fetch_doc(&client, "Deviasi", b).await?;
            print_diff(&style, a, b, &da, &db);
        }
        Some(other) => {
            eprintln!("command tidak dikenal: {}", other);
            print_help();
            std::process::exit(1);
        }
        None => {
            print_help();
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn fetch_meta(client: &RjssClient) -> Result<serde_json::Value, JssError> {
    let path = "/api/method/frappe.desk.form.load.getdoctype?doctype=Deviasi&with_parent=1";
    let raw = client.authenticated_get(path).await?;
    serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse meta: {e}")))
}

async fn fetch_mdn(client: &RjssClient, name: &str) -> Result<serde_json::Value, JssError> {
    let raw = client.get_doc("Master Data Nasabah", name).await?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse mdn: {e}")))?;
    Ok(v["data"].clone())
}

async fn fetch_all(
    client: &RjssClient,
    no_perjanjian: &str,
) -> Result<(serde_json::Value, RincianHutang), JssError> {
    let mdn = fetch_mdn(client, no_perjanjian).await?;
    let no_rek = mdn["no_rekening_kredit"].as_str().ok_or_else(|| {
        JssError::Validation(format!(
            "'{}' tidak punya no_rekening_kredit",
            no_perjanjian
        ))
    })?;
    let rincian = cek_hutang(client, no_rek).await?;
    Ok((mdn, rincian))
}

async fn fetch_doc(
    client: &RjssClient,
    doctype: &str,
    name: &str,
) -> Result<serde_json::Value, JssError> {
    let raw = client.get_doc(doctype, name).await?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse doc: {e}")))?;
    Ok(v["data"].clone())
}

async fn cek_hutang(client: &RjssClient, no_rekening: &str) -> Result<RincianHutang, JssError> {
    let raw = client
        .post_form(
            "/api/method/juragan.ops.doctype.master_data_nasabah.master_data_nasabah.cek_rincian_hutang_nasabah",
            &[("no_rekening_kredit", no_rekening)],
        )
        .await?;
    let resp: CekTunggakanResponse =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse hutang: {e}")))?;
    Ok(resp.message)
}

async fn fetch_history(
    client: &RjssClient,
    no_perjanjian: &str,
    limit: u32,
) -> Result<Vec<serde_json::Value>, JssError> {
    let raw = client
        .doctype("Deviasi")
        .filter("no_perjanjian", "=", no_perjanjian)
        .fields(vec![
            "name",
            "jenis_deviasi",
            "total_pengurangan",
            "status",
            "tanggal",
            "creation",
        ])
        .order_by("creation desc")
        .limit(limit)
        .execute_raw()
        .await?;
    parse_data(&raw)
}

async fn fetch_recent(client: &RjssClient, limit: u32) -> Result<Vec<serde_json::Value>, JssError> {
    let raw = client
        .doctype("Deviasi")
        .fields(vec![
            "name",
            "no_perjanjian",
            "jenis_deviasi",
            "total_pengurangan",
            "status",
            "tanggal",
        ])
        .order_by("creation desc")
        .limit(limit)
        .execute_raw()
        .await?;
    parse_data(&raw)
}

fn compute_scenario(jenis: &str, nilai: f64, r: &RincianHutang) -> Result<Scenario, JssError> {
    let (jenis_str, p_denda_req, p_bunga_req) = match jenis.to_lowercase().as_str() {
        "bunga" => ("Pengurangan Bunga", 0.0, nilai),
        "denda" => ("Pengurangan Denda", nilai, 0.0),
        "both" | "bunga_denda" | "keduanya" => {
            ("Pengurangan Bunga & Denda", nilai * 0.5, nilai * 0.5)
        }
        _ => {
            return Err(JssError::Validation(format!(
                "jenis '{}' tidak dikenal (bunga|denda|both)",
                jenis
            )));
        }
    };

    let p_bunga = p_bunga_req.min(r.sisa_bunga);
    let p_denda = p_denda_req.min(r.total_denda);

    let sisa_bunga_akhir = (r.sisa_bunga - p_bunga).max(0.0);
    let sisa_denda_akhir = (r.total_denda - p_denda).max(0.0);
    let sisa_admin_akhir = r.sisa_admin;
    let total_bayar =
        r.sisa_pokok + sisa_bunga_akhir + sisa_admin_akhir + sisa_denda_akhir - r.saldo_tabungan;

    let hemat = (r.sisa_bunga - sisa_bunga_akhir)
        + (r.total_denda - sisa_denda_akhir)
        + (r.sisa_admin - sisa_admin_akhir);

    Ok(Scenario {
        jenis: jenis_str.to_string(),
        pengurangan_bunga: p_bunga,
        pengurangan_denda: p_denda,
        total_pengurangan: p_bunga + p_denda,
        sisa_bunga_akhir,
        sisa_denda_akhir,
        total_bayar_nasabah: total_bayar,
        hemat,
    })
}

fn build_payload(
    no_perjanjian: &str,
    mdn: &serde_json::Value,
    r: &RincianHutang,
    sc: &Scenario,
    alasan: Option<&str>,
) -> Result<serde_json::Value, JssError> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut payload = serde_json::json!({
        "doctype": "Deviasi",
        "no_perjanjian": no_perjanjian,
        "nama_nasabah": mdn["nama_nasabah"].as_str().unwrap_or(""),
        "cabang": mdn["cabang"].as_str().unwrap_or(""),
        "no_hp": mdn["no_hp_1"].as_str().unwrap_or(""),
        "plafond": mdn["pinjaman"].as_f64().unwrap_or(0.0),
        "jenis_deviasi": sc.jenis,
        "tanggal": today,
        "waktu_cek_hutang": now,
        "sisa_os": r.total_hutang,
        "sisa_pokok": r.sisa_pokok,
        "sisa_bunga": r.sisa_bunga,
        "sisa_admin": r.sisa_admin,
        "total_sisa_angsuran": r.total_sisa_angsuran,
        "total_denda": r.total_denda,
        "saldo_tabungan": r.saldo_tabungan,
        "total_hutang": r.total_hutang,
        "pengurangan_denda": sc.pengurangan_denda,
        "pengurangan_bunga": sc.pengurangan_bunga,
        "pengurangan_admin": 0.0,
        "total_pengurangan": sc.total_pengurangan,
        "sisa_pokok_akhir": r.sisa_pokok,
        "sisa_bunga_akhir": sc.sisa_bunga_akhir,
        "sisa_admin_akhir": r.sisa_admin,
        "sisa_denda_akhir": sc.sisa_denda_akhir,
        "total_bayar_nasabah": sc.total_bayar_nasabah,
    });

    if let Some(a) = alasan {
        payload["alasan"] = serde_json::Value::String(a.to_string());
    }

    Ok(payload)
}

fn extract_fields(meta: &serde_json::Value) -> Vec<FieldDef> {
    let mut out = Vec::new();
    let Some(docs) = meta["docs"].as_array() else {
        return out;
    };
    let Some(deviasi) = docs.iter().find(|d| d["name"] == "Deviasi") else {
        return out;
    };
    let Some(fields) = deviasi["fields"].as_array() else {
        return out;
    };
    for f in fields {
        out.push(FieldDef {
            fieldname: f["fieldname"].as_str().unwrap_or("").to_string(),
            label: f["label"].as_str().map(String::from),
            fieldtype: f["fieldtype"].as_str().unwrap_or("").to_string(),
            reqd: f["reqd"].as_i64().unwrap_or(0) as i32,
            options: f["options"].as_str().map(String::from),
            hidden: f["hidden"].as_i64().unwrap_or(0) as i32,
            read_only: f["read_only"].as_i64().unwrap_or(0) as i32,
            depends_on: f["depends_on"].as_str().map(String::from),
        });
    }
    out
}

fn print_meta(style: &Style, meta: &serde_json::Value) {
    let fields = extract_fields(meta);
    println!("{} {} field", style.bold("Deviasi:"), fields.len());
    println!();
    for f in &fields {
        let req = if f.reqd == 1 {
            style.red(" *")
        } else {
            String::new()
        };
        let ro = if f.read_only == 1 {
            style.dim(" [ro]")
        } else {
            String::new()
        };
        let hid = if f.hidden == 1 {
            style.dim(" [hidden]")
        } else {
            String::new()
        };
        println!(
            "  {:<32} {:<14} {}{}{}{}",
            f.fieldname,
            f.fieldtype,
            f.label.as_deref().unwrap_or("-"),
            req,
            ro,
            hid,
        );
    }
}

fn print_schema(style: &Style, meta: &serde_json::Value, json_mode: bool) -> Result<(), JssError> {
    if json_mode {
        let s = serde_json::to_string_pretty(meta)
            .map_err(|e| JssError::Parse(format!("json: {e}")))?;
        println!("{}", s);
        return Ok(());
    }

    let fields = extract_fields(meta);

    println!("{}", style.bold("═══ SKEMA DEVIASI ═══"));
    println!();

    let required: Vec<_> = fields.iter().filter(|f| f.reqd == 1).collect();
    let visible: Vec<_> = fields
        .iter()
        .filter(|f| {
            f.hidden != 1 && !f.fieldtype.contains("Section") && !f.fieldtype.contains("Column")
        })
        .collect();

    println!("{}", style.bold("Field wajib:"));
    for f in &required {
        println!(
            "  {} {:<30} {:<12} {}",
            style.red("●"),
            f.fieldname,
            f.fieldtype,
            f.label.as_deref().unwrap_or("-"),
        );
    }
    println!();

    println!(
        "{}",
        style.bold(&format!("Field visible ({}):", visible.len()))
    );
    for f in &visible {
        let opt = f
            .options
            .as_ref()
            .map(|o| format!(" [{}]", o))
            .unwrap_or_default();
        println!(
            "  {:<32} {:<12} {}{}",
            f.fieldname,
            f.fieldtype,
            f.label.as_deref().unwrap_or("-"),
            opt,
        );
        if let Some(d) = &f.depends_on {
            println!("      {}", style.dim(&format!("depends_on: {}", d)));
        }
    }
    println!();

    let selects: Vec<_> = fields
        .iter()
        .filter(|f| f.fieldtype == "Select" && f.options.is_some())
        .collect();
    if !selects.is_empty() {
        println!("{}", style.bold("Field Select:"));
        for f in &selects {
            println!("  {}:", f.fieldname);
            for o in f.options.as_deref().unwrap_or("").split('\n') {
                if !o.trim().is_empty() {
                    println!("    - {}", o);
                }
            }
        }
    }

    Ok(())
}

fn validate_payload(style: &Style, meta: &serde_json::Value, payload: &serde_json::Value) {
    let fields = extract_fields(meta);
    let Some(obj) = payload.as_object() else {
        println!("{} payload bukan object", style.red("✗"));
        return;
    };

    let required: Vec<_> = fields
        .iter()
        .filter(|f| f.reqd == 1 && f.fieldtype != "Section Break" && f.fieldtype != "Column Break")
        .collect();

    let mut ok = true;
    println!("{}", style.bold("Validasi payload"));
    println!();
    println!("{}", style.bold("Field wajib:"));
    for f in &required {
        let val = obj.get(&f.fieldname);
        let empty = val
            .map(|v| v.is_null() || v.as_str() == Some(""))
            .unwrap_or(true);
        if empty {
            println!(
                "  {} {:<30} {}",
                style.red("✗"),
                f.fieldname,
                style.red("MISSING")
            );
            ok = false;
        } else {
            println!(
                "  {} {:<30} {}",
                style.green("✓"),
                f.fieldname,
                trunc(&val.unwrap().to_string(), 40)
            );
        }
    }
    println!();

    let unknown: Vec<_> = obj
        .keys()
        .filter(|k| {
            !fields.iter().any(|f| &f.fieldname == *k)
                && !matches!(
                    k.as_str(),
                    "doctype"
                        | "name"
                        | "owner"
                        | "creation"
                        | "modified"
                        | "modified_by"
                        | "idx"
                        | "docstatus"
                )
        })
        .collect();

    if !unknown.is_empty() {
        println!("{}", style.bold("Field tidak dikenal:"));
        for k in unknown {
            println!("  {} {}", style.yellow("?"), k);
        }
        println!();
    }

    if ok {
        println!("{}", style.green("✓ Payload valid (struktur)"));
    } else {
        println!("{}", style.red("✗ Payload kurang lengkap"));
    }
}

fn print_mdn(style: &Style, np: &str, mdn: &serde_json::Value) {
    println!("{}", style.bold("Master Data Nasabah"));
    println!("  name               : {}", style.cyan(np));
    println!(
        "  nama_nasabah       : {}",
        mdn["nama_nasabah"].as_str().unwrap_or("-")
    );
    println!(
        "  no_rekening_kredit : {}",
        mdn["no_rekening_kredit"].as_str().unwrap_or("-")
    );
    println!(
        "  register_id        : {}",
        mdn["register_id"].as_str().unwrap_or("-")
    );
    println!(
        "  status             : {}",
        mdn["status"].as_str().unwrap_or("-")
    );
    println!(
        "  status_piutang     : {}",
        mdn["status_piutang"].as_str().unwrap_or("-")
    );
    println!(
        "  status_bpkb        : {}",
        mdn["status_bpkb"].as_str().unwrap_or("-")
    );
    println!(
        "  cabang             : {}",
        mdn["cabang"].as_str().unwrap_or("-")
    );
    println!(
        "  no_hp_1            : {}",
        mdn["no_hp_1"].as_str().unwrap_or("-")
    );
    println!(
        "  nopol              : {}",
        mdn["nopol"].as_str().unwrap_or("-")
    );
    println!(
        "  pinjaman           : {}",
        rupiah(mdn["pinjaman"].as_f64().unwrap_or(0.0))
    );
    println!(
        "  angsuran           : {}",
        rupiah(mdn["angsuran"].as_f64().unwrap_or(0.0))
    );
}

fn print_hutang_report(style: &Style, np: &str, mdn: &serde_json::Value, r: &RincianHutang) {
    println!();
    println!("{}", style.bold("═══ RINCIAN HUTANG ═══"));
    println!("  No Perjanjian : {}", style.cyan(np));
    println!(
        "  Nasabah       : {}",
        mdn["nama_nasabah"].as_str().unwrap_or("-")
    );
    println!(
        "  Cabang        : {}",
        mdn["cabang"].as_str().unwrap_or("-")
    );
    println!(
        "  Status        : {}",
        mdn["status"].as_str().unwrap_or("-")
    );
    println!();

    println!("{}", style.bold("Ringkasan"));
    println!("  {:<24} {:>16}", "Sisa Pokok", rupiah(r.sisa_pokok));
    println!("  {:<24} {:>16}", "Sisa Bunga", rupiah(r.sisa_bunga));
    println!("  {:<24} {:>16}", "Sisa Admin", rupiah(r.sisa_admin));
    println!("  {:<24} {:>16}", "Total Denda", rupiah(r.total_denda));
    println!(
        "  {:<24} {:>16}",
        "Saldo Tabungan",
        rupiah(r.saldo_tabungan)
    );
    println!("  {}", style.dim(&"-".repeat(42)));
    println!(
        "  {:<24} {:>16}",
        style.bold("Total Hutang"),
        style.bold(&rupiah(r.total_hutang))
    );
    println!();

    println!("  {} baris angsuran dari server", r.tabel_angsuran.len());
}

fn print_simulasi(
    style: &Style,
    np: &str,
    mdn: &serde_json::Value,
    r: &RincianHutang,
    sc: &Scenario,
    payload: &serde_json::Value,
) {
    println!();
    println!("{}", style.bold("═══ SIMULASI DEVIASI ═══"));
    println!();
    println!("{}", style.bold("Identitas"));
    println!("  No Perjanjian : {}", style.cyan(np));
    println!(
        "  Nasabah       : {}",
        mdn["nama_nasabah"].as_str().unwrap_or("-")
    );
    println!(
        "  Cabang        : {}",
        mdn["cabang"].as_str().unwrap_or("-")
    );
    println!(
        "  No HP         : {}",
        mdn["no_hp_1"].as_str().unwrap_or("-")
    );
    println!();

    println!("{}", style.bold("Input"));
    println!("  Jenis Deviasi    : {}", sc.jenis);
    println!(
        "  Nilai Pengajuan  : {}",
        rupiah(sc.pengurangan_bunga + sc.pengurangan_denda)
    );
    println!();

    println!("{}", style.bold("Perhitungan (dari server)"));
    println!("  {:<28} {:>16}", "sisa_pokok", rupiah(r.sisa_pokok));
    println!("  {:<28} {:>16}", "sisa_bunga", rupiah(r.sisa_bunga));
    println!("  {:<28} {:>16}", "sisa_admin", rupiah(r.sisa_admin));
    println!("  {:<28} {:>16}", "total_denda", rupiah(r.total_denda));
    println!(
        "  {:<28} {:>16}",
        "saldo_tabungan",
        rupiah(r.saldo_tabungan)
    );
    println!("  {:<28} {:>16}", "total_hutang", rupiah(r.total_hutang));
    println!();

    println!("{}", style.bold("Pengurangan"));
    if sc.pengurangan_bunga > 0.0 {
        println!(
            "  {:<28} {:>16}",
            "pengurangan_bunga",
            rupiah(sc.pengurangan_bunga)
        );
    }
    if sc.pengurangan_denda > 0.0 {
        println!(
            "  {:<28} {:>16}",
            "pengurangan_denda",
            rupiah(sc.pengurangan_denda)
        );
    }
    println!(
        "  {:<28} {:>16}",
        style.bold("total_pengurangan"),
        style.bold(&rupiah(sc.total_pengurangan))
    );
    println!();

    if (sc.total_pengurangan - (sc.pengurangan_bunga + sc.pengurangan_denda)).abs() > 0.01 {
        println!(
            "  {}",
            style.yellow("Catatan: sebagian pengurangan melebihi nilai yang ada, jadi diklamp ke nilai aktual.")
        );
        println!();
    }

    println!("{}", style.bold("Hasil"));
    println!("  {:<28} {:>16}", "sisa_pokok_akhir", rupiah(r.sisa_pokok));
    println!(
        "  {:<28} {:>16}",
        "sisa_bunga_akhir",
        rupiah(sc.sisa_bunga_akhir)
    );
    println!("  {:<28} {:>16}", "sisa_admin_akhir", rupiah(r.sisa_admin));
    println!(
        "  {:<28} {:>16}",
        "sisa_denda_akhir",
        rupiah(sc.sisa_denda_akhir)
    );
    println!("  {}", style.dim(&"-".repeat(46)));
    println!(
        "  {:<28} {:>16}",
        style.bold("total_bayar_nasabah"),
        style.green(&style.bold(&rupiah(sc.total_bayar_nasabah)))
    );
    println!(
        "  {:<28} {:>16}",
        style.dim("hemat"),
        style.dim(&rupiah(sc.hemat))
    );
    println!();

    println!("{}", style.bold("Payload JSON yang akan dikirim:"));
    println!("{}", serde_json::to_string_pretty(payload).unwrap());
}

fn print_scenario(style: &Style, np: &str, mdn: &serde_json::Value, r: &RincianHutang, nilai: f64) {
    println!();
    println!("{}", style.bold(&format!("═══ SKENARIO untuk {} ═══", np)));
    println!(
        "  Nasabah : {}",
        mdn["nama_nasabah"].as_str().unwrap_or("-")
    );
    println!("  Nilai   : {} (per skenario)", rupiah(nilai));
    println!();

    println!(
        "  {:<28}  {:>14}  {:>16}  {:>14}",
        "Jenis", "Pengurangan", "Bayar Nasabah", "Hemat"
    );
    println!("  {}", style.dim(&"─".repeat(80)));

    let mut hasil: Vec<Scenario> = Vec::new();
    for jenis in &["bunga", "denda", "both"] {
        if let Ok(sc) = compute_scenario(jenis, nilai, r) {
            hasil.push(sc);
        }
    }

    let min_bayar = hasil
        .iter()
        .map(|s| s.total_bayar_nasabah)
        .fold(f64::MAX, f64::min);

    for sc in &hasil {
        let is_best = (sc.total_bayar_nasabah - min_bayar).abs() < 0.01;
        let marker = if is_best {
            style.green(" ←")
        } else {
            String::new()
        };
        let jenis_str = if is_best {
            style.green(&sc.jenis)
        } else {
            sc.jenis.clone()
        };
        println!(
            "  {:<28}  {:>14}  {:>16}  {:>14}{}",
            jenis_str,
            rupiah(sc.total_pengurangan),
            rupiah(sc.total_bayar_nasabah),
            rupiah(sc.hemat),
            marker,
        );
    }
    println!();

    println!("{}", style.bold("Kontek"));
    println!("  Sisa Bunga  : {}", rupiah(r.sisa_bunga));
    println!("  Total Denda : {}", rupiah(r.total_denda));
    println!();

    if r.total_denda == 0.0 {
        println!(
            "  {}",
            style.yellow("Tidak ada denda. Skenario 'denda' tidak akan mengurangi apapun.")
        );
    }
}

fn print_history(
    style: &Style,
    np: &str,
    items: &[serde_json::Value],
    json_mode: bool,
) -> Result<(), JssError> {
    if json_mode {
        let s = serde_json::to_string_pretty(&items)
            .map_err(|e| JssError::Parse(format!("json: {e}")))?;
        println!("{}", s);
        return Ok(());
    }

    println!();
    println!(
        "{}",
        style.bold(&format!("═══ HISTORY DEVIASI: {} ═══", np))
    );
    println!();

    if items.is_empty() {
        println!("  {}", style.dim("Belum ada Deviasi untuk nasabah ini."));
        return Ok(());
    }

    println!(
        "  {:<22}  {:<28}  {:>14}  {:<14}  tanggal",
        "name", "jenis", "pengurangan", "status"
    );
    println!("  {}", style.dim(&"─".repeat(98)));

    for it in items {
        let status_colored = match it["status"].as_str().unwrap_or("") {
            "Approved" | "Disetujui" | "Selesai" => {
                style.green(it["status"].as_str().unwrap_or("-"))
            }
            "Rejected" | "Ditolak" => style.red(it["status"].as_str().unwrap_or("-")),
            "Draft" => style.dim(it["status"].as_str().unwrap_or("-")),
            s => style.yellow(s),
        };
        println!(
            "  {:<22}  {:<28}  {:>14}  {:<14}  {}",
            it["name"].as_str().unwrap_or("-"),
            trunc(it["jenis_deviasi"].as_str().unwrap_or("-"), 28),
            rupiah(it["total_pengurangan"].as_f64().unwrap_or(0.0)),
            status_colored,
            it["tanggal"].as_str().unwrap_or("-"),
        );
    }
    println!();
    println!("  {} dokumen", items.len());

    Ok(())
}

fn print_diff(style: &Style, a: &str, b: &str, da: &serde_json::Value, db: &serde_json::Value) {
    println!("{}", style.bold(&format!("Diff {} vs {}", a, b)));
    println!();

    let (Some(oa), Some(ob)) = (da.as_object(), db.as_object()) else {
        return;
    };

    let mut keys: Vec<&String> = oa.keys().chain(ob.keys()).collect();
    keys.sort();
    keys.dedup();

    let skip = [
        "creation",
        "modified",
        "modified_by",
        "owner",
        "name",
        "idx",
        "doctype",
    ];

    let mut ada = false;
    for k in keys {
        if skip.contains(&k.as_str()) {
            continue;
        }
        let va = oa.get(k);
        let vb = ob.get(k);
        if va != vb {
            ada = true;
            println!("  {}", style.bold(k));
            println!("    A: {}", style.red(&trunc(&fmt_val(va), 70)));
            println!("    B: {}", style.green(&trunc(&fmt_val(vb), 70)));
        }
    }

    if !ada {
        println!("  {}", style.dim("Tidak ada perbedaan"));
    }
}

async fn run_batch(
    client: &RjssClient,
    style: &Style,
    file: &str,
    jenis: &str,
    nilai: f64,
    csv_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let list: Vec<String> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();

    if list.is_empty() {
        eprintln!("file {} kosong", file);
        return Ok(());
    }

    if csv_mode {
        println!(
            "no_perjanjian,nama_nasabah,cabang,sisa_pokok,sisa_bunga,sisa_admin,total_denda,total_hutang,jenis,pengurangan,total_bayar,hemat"
        );
    } else {
        println!(
            "{}  {} nasabah dari {} (jenis: {}, nilai: {})",
            style.bold("Batch:"),
            list.len(),
            file,
            jenis,
            rupiah(nilai)
        );
        println!();
        println!(
            "{:<28}  {:<24}  {:>14}  {:>14}  {:>14}",
            "no_perjanjian", "nasabah", "total_hutang", "pengurangan", "bayar"
        );
        println!("  {}", style.dim(&"─".repeat(100)));
    }

    let mut ok = 0usize;
    let mut err = 0usize;

    for np in &list {
        match fetch_all(client, np).await {
            Ok((mdn, r)) => match compute_scenario(jenis, nilai, &r) {
                Ok(sc) => {
                    if csv_mode {
                        println!(
                            "{},{},{},{:.0},{:.0},{:.0},{:.0},{:.0},{},{:.0},{:.0},{:.0}",
                            csv_escape(np),
                            csv_escape(mdn["nama_nasabah"].as_str().unwrap_or("")),
                            csv_escape(mdn["cabang"].as_str().unwrap_or("")),
                            r.sisa_pokok,
                            r.sisa_bunga,
                            r.sisa_admin,
                            r.total_denda,
                            r.total_hutang,
                            sc.jenis,
                            sc.total_pengurangan,
                            sc.total_bayar_nasabah,
                            sc.hemat,
                        );
                    } else {
                        println!(
                            "{:<28}  {:<24}  {:>14}  {:>14}  {:>14}",
                            np,
                            trunc(mdn["nama_nasabah"].as_str().unwrap_or("-"), 24),
                            rupiah(r.total_hutang),
                            rupiah(sc.total_pengurangan),
                            rupiah(sc.total_bayar_nasabah),
                        );
                    }
                    ok += 1;
                }
                Err(e) => {
                    if csv_mode {
                        eprintln!("skip {}: {}", np, e);
                    } else {
                        println!("{:<28}  {}", np, style.red(&format!("ERROR: {}", e)));
                    }
                    err += 1;
                }
            },
            Err(e) => {
                if csv_mode {
                    eprintln!("skip {}: {}", np, e);
                } else {
                    println!("{:<28}  {}", np, style.red(&format!("ERROR: {}", e)));
                }
                err += 1;
            }
        }
    }

    if !csv_mode {
        println!();
        println!(
            "  {} ok, {} error",
            style.green(&ok.to_string()),
            if err > 0 {
                style.red(&err.to_string())
            } else {
                err.to_string()
            }
        );
    }

    Ok(())
}

fn print_help() {
    println!("deviasi_dev — tool development Deviasi (READ-ONLY, tidak kirim POST)");
    println!();
    println!("USAGE:");
    println!("  deviasi_dev <command> [args] [--json|--csv]");
    println!();
    println!("COMMANDS:");
    println!("  schema                              ringkasan field wajib, visible, select");
    println!("  meta                                semua field + tipe + reqd");
    println!("  cek <no_perjanjian>                 Master Data Nasabah");
    println!("  hutang <no_perjanjian>              rincian hutang dari server");
    println!("  simulasi <np> <jenis> <nilai> [alasan]");
    println!("                                      payload lengkap + perhitungan");
    println!("  scenario <np> <nilai>               bandingkan bunga|denda|both");
    println!("  history <np> [limit]                riwayat Deviasi untuk nasabah");
    println!("  batch <file> [jenis] [nilai]        batch dari daftar np per baris");
    println!("  export <np> <jenis> <nilai> [dir]   simpan payload ke file");
    println!("  validate [file.json]                validasi terhadap meta");
    println!("  explore                             schema + 5 Deviasi terbaru");
    println!("  diff <dev1> <dev2>                  bandingkan 2 dokumen");
    println!();
    println!("JENIS:");
    println!("  bunga    Pengurangan Bunga");
    println!("  denda    Pengurangan Denda");
    println!("  both     Pengurangan Bunga & Denda (dibagi 2)");
    println!();
    println!("EXAMPLES:");
    println!("  deviasi_dev schema");
    println!("  deviasi_dev hutang SP-26-6001-0000053-0001");
    println!("  deviasi_dev simulasi SP-26-6001-0000053-0001 bunga 500000");
    println!("  deviasi_dev scenario SP-26-6001-0000053-0001 500000");
    println!("  deviasi_dev batch nasabah.txt bunga 100000 --csv > hasil.csv");
    println!(
        "  deviasi_dev export SP-26-6001-0000053-0001 bunga 500000 ./out \"Alasan pengurangan\""
    );
    println!("  deviasi_dev history SP-26-6001-0000053-0001");
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn parse_data<T: serde::de::DeserializeOwned>(raw: &str) -> Result<Vec<T>, JssError> {
    #[derive(Deserialize)]
    struct Envelope<T> {
        data: Vec<T>,
    }
    let env: Envelope<T> =
        serde_json::from_str(raw).map_err(|e| JssError::Parse(format!("parse_data: {e}")))?;
    Ok(env.data)
}

fn fmt_val(v: Option<&serde_json::Value>) -> String {
    match v {
        None => "(missing)".into(),
        Some(serde_json::Value::Null) => "(null)".into(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn trunc(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        format!(
            "{}…",
            chars.iter().take(n.saturating_sub(1)).collect::<String>()
        )
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn rupiah(v: f64) -> String {
    let negatif = v < 0.0;
    let n = v.abs().round() as i64;
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    let s: String = out.chars().rev().collect();
    if negatif { format!("-{}", s) } else { s }
}
