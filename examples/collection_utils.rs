use librjss::{ClientConfig, JssError, RjssClient};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

// ============================================================================
// ============================================================================

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

#[derive(Debug, Serialize, Deserialize)]
struct CekTunggakanResponse {
    message: RincianHutang,
}

#[derive(Debug, Serialize, Deserialize)]
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
    tabel_angsuran: Vec<BarisAngsuran>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BarisAngsuran {
    #[serde(rename = "No")]
    no: i64,
    #[serde(rename = "J_Total")]
    j_total: String,
    #[serde(rename = "J_Pokok")]
    j_pokok: String,
    #[serde(rename = "J_Bunga")]
    j_bunga: String,
    #[serde(rename = "J_Adm")]
    j_adm: String,
    #[serde(rename = "Sisa_Pokok")]
    sisa_pokok: String,
    #[serde(rename = "Sisa_Bunga")]
    sisa_bunga: String,
    #[serde(rename = "Sisa_Admin")]
    sisa_admin: String,
    #[serde(rename = "Total_Sisa_Angsuran")]
    total_sisa_angsuran: String,
    #[serde(rename = "Tgl_Jatuh_Tempo")]
    tgl_jatuh_tempo: String,
    #[serde(rename = "Tgl_Lunas_Full")]
    tgl_lunas_full: String,
    #[serde(rename = "Hari_Telat")]
    hari_telat: i64,
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Debug, Serialize, PartialEq)]
enum StatusKesehatan {
    Sehat,
    Perhatian,
    Bahaya,
}

#[derive(Debug, Serialize)]
struct Analisa {
    status: StatusKesehatan,
    total_angsuran: usize,
    lunas: usize,
    belum_bayar: usize,
    telat: usize,
    jatuh_tempo_terdekat: Option<String>,
    persentase_progress: f64,
}

impl Analisa {
    fn dari(r: &RincianHutang) -> Self {
        let total = r.tabel_angsuran.len();
        let lunas = r
            .tabel_angsuran
            .iter()
            .filter(|b| b.status == "LUNAS")
            .count();
        let telat = r.tabel_angsuran.iter().filter(|b| b.hari_telat > 0).count();
        let belum = total - lunas;

        let jt_terdekat = r
            .tabel_angsuran
            .iter()
            .filter(|b| b.status != "LUNAS")
            .min_by_key(|b| parse_date_key(&b.tgl_jatuh_tempo))
            .map(|b| b.tgl_jatuh_tempo.clone());

        let status = if telat >= 3 {
            StatusKesehatan::Bahaya
        } else if telat >= 1 || total > 0 && (belum as f64 / total as f64) > 0.5 {
            StatusKesehatan::Perhatian
        } else {
            StatusKesehatan::Sehat
        };

        let progress = if total > 0 {
            (lunas as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Analisa {
            status,
            total_angsuran: total,
            lunas,
            belum_bayar: belum,
            telat,
            jatuh_tempo_terdekat: jt_terdekat,
            persentase_progress: progress,
        }
    }
}

fn parse_date_key(s: &str) -> (i32, u32, u32) {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 {
        return (9999, 12, 31);
    }
    let d = parts[0].parse().unwrap_or(31);
    let m = parts[1].parse().unwrap_or(12);
    let y = parts[2].parse().unwrap_or(9999);
    (y, m, d)
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
    let csv_mode = has_flag(&raw_args, "--csv");
    let quiet = has_flag(&raw_args, "--quiet");
    let batch_file = extract_flag_value(&raw_args, "--batch");

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
        Some("cek-tunggakan") | Some("ct") => {
            if let Some(file) = batch_file {
                run_batch(&client, &style, &file, csv_mode).await?;
            } else {
                let no_rek = args.get(1).ok_or("butuh no_rekening_kredit")?;
                let rincian = cek_tunggakan(&client, no_rek).await?;
                render_cek_tunggakan(&style, no_rek, &rincian, json_mode, csv_mode, quiet)?;
            }
        }
        Some("pic-belum-lapor") | Some("pbl") => {
            let v = call_empty_filters(
                &client,
                "juragan.collection.utils.get_pic_belum_lapor_hari_ini",
            )
            .await?;
            render_generic(&style, "PIC Belum Lapor Hari Ini", &v, json_mode, csv_mode)?;
        }
        Some("dokumen-belum-konfirmasi") | Some("dbk") => {
            let v = call_empty_filters(
                &client,
                "juragan.collection.utils.get_dokumen_belum_dikonfirmasi",
            )
            .await?;
            render_generic(
                &style,
                "Dokumen Belum Dikonfirmasi",
                &v,
                json_mode,
                csv_mode,
            )?;
        }
        Some("dokumen-overdue") | Some("do") => {
            let v = call_empty_filters(
                &client,
                "juragan.collection.utils.get_dokumen_overdue_collection",
            )
            .await?;
            render_generic(
                &style,
                "Dokumen Overdue Collection",
                &v,
                json_mode,
                csv_mode,
            )?;
        }
        Some("combined") | Some("comb") => {
            let from = args.get(1).ok_or("butuh date_from (YYYY-MM-DD)")?;
            let to = args.get(2).ok_or("butuh date_to (YYYY-MM-DD)")?;
            let v = call_date_range(&client, from, to).await?;
            render_generic(
                &style,
                &format!("Combined Collection {} - {}", from, to),
                &v,
                json_mode,
                csv_mode,
            )?;
        }
        Some("list-whitelist") => print_whitelist(&style),
        Some("pdf") => {
            let doctype = args.get(1).ok_or("butuh doctype")?;
            let name = args.get(2).ok_or("butuh docname")?;
            let format = args
                .get(3)
                .map(String::as_str)
                .unwrap_or("Form Rincian Sisa Piutang Nasabah");
            let out_dir = std::path::PathBuf::from(args.get(4).map(String::as_str).unwrap_or("."));

            std::fs::create_dir_all(&out_dir)?;
            let safe_name = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            let safe_format = format.replace(['/', '\\'], "_");
            let path = out_dir.join(format!("{}-{}.pdf", safe_format, safe_name));

            let bytes = client
                .download_pdf_kartu_piutang(doctype, name, format, false)
                .await?;

            std::fs::write(&path, &bytes)?;
            println!(
                "  {} {} ({} bytes)",
                style.green("✓"),
                path.display(),
                bytes.len()
            );
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn extract_flag_value(args: &[String], flag: &str) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == flag {
            return args.get(i + 1).cloned();
        }
        if let Some(rest) = a.strip_prefix(&format!("{}=", flag)) {
            return Some(rest.to_string());
        }
    }
    None
}

fn print_help() {
    println!("collection_utils — read-only CLI untuk Juragan Subsystem");
    println!();
    println!("USAGE:");
    println!("  collection_utils <command> [args] [flags]");
    println!();
    println!("COMMANDS:");
    println!("  cek-tunggakan <no_rekening_kredit>     rincian hutang nasabah");
    println!("  cek-tunggakan --batch <file.txt>       batch dari daftar no rekening");
    println!("  pic-belum-lapor                        PIC yang belum lapor hari ini");
    println!("  dokumen-belum-konfirmasi               dokumen menunggu konfirmasi");
    println!("  dokumen-overdue                        dokumen overdue collection");
    println!("  combined <date_from> <date_to>         data collection gabungan");
    println!("  list-whitelist                         endpoint read-only yang diizinkan");
    println!();
    println!("FLAGS:");
    println!("  --json          output JSON");
    println!("  --csv           output CSV (untuk Excel)");
    println!("  --quiet         hanya angka total_hutang");
    println!("  --batch <file>  baca daftar no rekening dari file");
    println!();
    println!("EXAMPLES:");
    println!("  collection_utils ct 6001-0000053-0001");
    println!("  collection_utils ct --batch nasabah.txt --csv > hasil.csv");
    println!("  collection_utils ct 6001-0000053-0001 --json | jq .total_hutang");
    println!("  collection_utils pbl");
    println!("  collection_utils combined 2026-09-09 2026-09-15");
}

fn print_whitelist(style: &Style) {
    let paths = [
        "/api/method/juragan.collection.utils.get_pic_belum_lapor_hari_ini",
        "/api/method/juragan.collection.utils.get_dokumen_belum_dikonfirmasi",
        "/api/method/juragan.collection.utils.get_dokumen_overdue_collection",
        "/api/method/juragan.collection.utils.get_combined_collection_data",
        "/api/method/juragan.ops.doctype.master_data_nasabah.master_data_nasabah.cek_rincian_hutang_nasabah",
    ];
    println!(
        "{}",
        style.bold("Endpoint read-only yang diizinkan lewat guard:")
    );
    for p in paths {
        println!("  {}", p);
    }
    println!();
    println!(
        "{}",
        style.dim("Semua operasi write tetap diblokir di sisi client.")
    );
}

async fn cek_tunggakan(
    client: &RjssClient,
    no_rekening_kredit: &str,
) -> Result<RincianHutang, JssError> {
    let raw = client
        .post_form(
            "/api/method/juragan.ops.doctype.master_data_nasabah.master_data_nasabah.cek_rincian_hutang_nasabah",
            &[("no_rekening_kredit", no_rekening_kredit)],
        )
        .await?;

    let resp: CekTunggakanResponse =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse: {e}")))?;
    Ok(resp.message)
}

async fn call_empty_filters(
    client: &RjssClient,
    method: &str,
) -> Result<serde_json::Value, JssError> {
    let path = format!("/api/method/{}", method);
    let raw = client.post_form(&path, &[("filters", "[]")]).await?;
    serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse: {e}")))
}

async fn call_date_range(
    client: &RjssClient,
    from: &str,
    to: &str,
) -> Result<serde_json::Value, JssError> {
    let raw = client
        .post_form(
            "/api/method/juragan.collection.utils.get_combined_collection_data",
            &[("date_from", from), ("date_to", to)],
        )
        .await?;
    serde_json::from_str(&raw).map_err(|e| JssError::Parse(format!("parse: {e}")))
}

async fn run_batch(
    client: &RjssClient,
    style: &Style,
    file: &str,
    csv_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let f = std::fs::File::open(file)?;
    let reader = std::io::BufReader::new(f);

    let mut list: Vec<String> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let s = line.trim();
        if !s.is_empty() && !s.starts_with('#') {
            list.push(s.to_string());
        }
    }

    if list.is_empty() {
        eprintln!("file {} kosong", file);
        return Ok(());
    }

    if csv_mode {
        println!(
            "no_rekening,csp_name,sisa_pokok,sisa_bunga,sisa_admin,total_denda,total_hutang,saldo_tabungan,total_angsuran,lunas,belum,telat,status,jatuh_tempo_terdekat"
        );
    } else {
        println!(
            "{}  {} nasabah dari {}",
            style.bold("Batch:"),
            list.len(),
            file
        );
        println!();
        println!(
            "{:<22}  {:<24}  {:>14}  {:>7}  {:>7}  {:<10}",
            "no_rekening", "csp_name", "total_hutang", "lunas", "belum", "status"
        );
        println!("  {}", style.dim(&"─".repeat(88)));
    }

    let mut ok = 0usize;
    let mut err = 0usize;

    for no_rek in &list {
        match cek_tunggakan(client, no_rek).await {
            Ok(r) => {
                let a = Analisa::dari(&r);
                if csv_mode {
                    println!(
                        "{},{},{:.0},{:.0},{:.0},{:.0},{:.0},{:.0},{},{},{},{},{},{}",
                        no_rek,
                        csv_escape(r.csp_name.as_deref().unwrap_or("")),
                        r.sisa_pokok,
                        r.sisa_bunga,
                        r.sisa_admin,
                        r.total_denda,
                        r.total_hutang,
                        r.saldo_tabungan,
                        a.total_angsuran,
                        a.lunas,
                        a.belum_bayar,
                        a.telat,
                        status_str(&a.status),
                        a.jatuh_tempo_terdekat.as_deref().unwrap_or(""),
                    );
                } else {
                    let status_colored = color_status(style, &a.status);
                    println!(
                        "{:<22}  {:<24}  {:>14}  {:>7}  {:>7}  {}",
                        no_rek,
                        truncate(r.csp_name.as_deref().unwrap_or("-"), 24),
                        rupiah(r.total_hutang),
                        a.lunas,
                        a.belum_bayar,
                        status_colored,
                    );
                }
                ok += 1;
            }
            Err(e) => {
                if csv_mode {
                    eprintln!("skip {}: {}", no_rek, e);
                } else {
                    println!("{:<22}  {}", no_rek, style.red(&format!("ERROR: {}", e)));
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

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let t: String = chars.iter().take(n.saturating_sub(1)).collect();
        format!("{}…", t)
    }
}

fn status_str(s: &StatusKesehatan) -> &'static str {
    match s {
        StatusKesehatan::Sehat => "SEHAT",
        StatusKesehatan::Perhatian => "PERHATIAN",
        StatusKesehatan::Bahaya => "BAHAYA",
    }
}

fn color_status(style: &Style, s: &StatusKesehatan) -> String {
    match s {
        StatusKesehatan::Sehat => style.green("SEHAT"),
        StatusKesehatan::Perhatian => style.yellow("PERHATIAN"),
        StatusKesehatan::Bahaya => style.red("BAHAYA"),
    }
}

fn parse_amt(s: &str) -> f64 {
    s.replace(',', "").trim().parse().unwrap_or(0.0)
}

fn pct_of(part: f64, total: f64) -> f64 {
    if total > 0.0 {
        part / total * 100.0
    } else {
        0.0
    }
}

fn mini_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    format!(
        "{}{}",
        "█".repeat(filled.min(width)),
        "░".repeat(width.saturating_sub(filled))
    )
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

fn render_cek_tunggakan(
    style: &Style,
    no_rek: &str,
    r: &RincianHutang,
    json_mode: bool,
    csv_mode: bool,
    quiet: bool,
) -> Result<(), JssError> {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(r).unwrap());
        return Ok(());
    }

    let a = Analisa::dari(r);

    if csv_mode {
        println!(
            "no_rekening,csp_name,sisa_pokok,sisa_bunga,sisa_admin,total_denda,total_hutang,saldo_tabungan,total_angsuran,lunas,belum,telat,status,jatuh_tempo_terdekat"
        );
        println!(
            "{},{},{:.0},{:.0},{:.0},{:.0},{:.0},{:.0},{},{},{},{},{},{}",
            no_rek,
            csv_escape(r.csp_name.as_deref().unwrap_or("")),
            r.sisa_pokok,
            r.sisa_bunga,
            r.sisa_admin,
            r.total_denda,
            r.total_hutang,
            r.saldo_tabungan,
            a.total_angsuran,
            a.lunas,
            a.belum_bayar,
            a.telat,
            status_str(&a.status),
            a.jatuh_tempo_terdekat.as_deref().unwrap_or(""),
        );
        return Ok(());
    }

    if quiet {
        println!("{}", r.total_hutang);
        return Ok(());
    }

    let lunas_rows: Vec<&BarisAngsuran> = r
        .tabel_angsuran
        .iter()
        .filter(|b| b.status == "LUNAS")
        .collect();
    let total_pokok: f64 = lunas_rows.iter().map(|b| parse_amt(&b.j_pokok)).sum();
    let total_bunga: f64 = lunas_rows.iter().map(|b| parse_amt(&b.j_bunga)).sum();
    let total_admin: f64 = lunas_rows.iter().map(|b| parse_amt(&b.j_adm)).sum();
    let grand = total_pokok + total_bunga + total_admin;
    let kontrak_total = grand + r.total_hutang;

    let pertama = r
        .tabel_angsuran
        .first()
        .map(|b| b.tgl_jatuh_tempo.as_str())
        .unwrap_or("-");
    let terakhir = r
        .tabel_angsuran
        .last()
        .map(|b| b.tgl_jatuh_tempo.as_str())
        .unwrap_or("-");
    let total_hari_telat: i64 = r.tabel_angsuran.iter().map(|b| b.hari_telat).sum();
    let tepat_waktu = r
        .tabel_angsuran
        .iter()
        .filter(|b| b.status == "LUNAS" && b.hari_telat == 0)
        .count();
    let angsuran_per_bulan = if a.total_angsuran > 0 {
        kontrak_total / a.total_angsuran as f64
    } else {
        0.0
    };

    let sep = "═".repeat(72);

    println!();
    println!("{}", style.bold(&sep));
    println!("{}", style.bold("  LAPORAN RINCIAN HUTANG NASABAH"));
    println!("{}", style.bold(&sep));
    println!();

    println!("{}", style.bold("▌ IDENTITAS"));
    println!("  {:<18} {}", "No Rekening", style.cyan(no_rek));
    println!(
        "  {:<18} {}",
        "Nama CSP",
        r.csp_name.as_deref().unwrap_or("-")
    );
    println!("  {:<18} {}", "Status", color_status(style, &a.status));
    println!();

    println!("{}", style.bold("▌ RINGKASAN KEUANGAN"));
    let total_sisa = r.total_hutang;
    let row = |label: &str, v: f64, show_pct: bool| {
        if show_pct && total_sisa > 0.0 {
            let pct_str = format!("{:>5.1}%", pct_of(v, total_sisa));
            println!(
                "  {:<18} {:>16}   {}",
                label,
                rupiah(v),
                style.dim(&pct_str)
            );
        } else {
            println!("  {:<18} {:>16}", label, rupiah(v));
        }
    };
    row("Sisa Pokok", r.sisa_pokok, true);
    row("Sisa Bunga", r.sisa_bunga, true);
    row("Sisa Admin", r.sisa_admin, true);
    row("Total Denda", r.total_denda, true);
    row("Saldo Tabungan", r.saldo_tabungan, false);
    println!("  {}", style.dim(&"─".repeat(50)));
    println!(
        "  {:<18} {:>16}",
        style.bold("TOTAL HUTANG"),
        style.bold(&rupiah(r.total_hutang))
    );
    println!();

    println!("{}", style.bold("▌ PROGRESS PEMBAYARAN"));
    let bar = mini_bar(a.persentase_progress, 40);
    println!(
        "  {}  {}/{}  {:.1}%",
        style.green(&bar),
        a.lunas,
        a.total_angsuran,
        a.persentase_progress
    );
    println!(
        "  {} {}    {} {}",
        style.dim("Sudah dibayar:"),
        rupiah(grand),
        style.dim("Sisa tagihan:"),
        rupiah(r.total_hutang),
    );
    println!(
        "  {} {}",
        style.dim("Kontrak total:"),
        rupiah(kontrak_total)
    );
    println!();

    println!("{}", style.bold("▌ STATISTIK"));
    println!("  {:<20} {}", "Angsuran Pertama", pertama);
    println!("  {:<20} {}", "Angsuran Terakhir", terakhir);
    println!("  {:<20} {} bulan", "Tenor", a.total_angsuran);
    println!(
        "  {:<20} {}",
        "Angsuran / bulan",
        rupiah(angsuran_per_bulan)
    );
    println!("  {:<20} {} hari", "Total Hari Telat", total_hari_telat);
    println!(
        "  {:<20} {}/{} ({:.0}%)",
        "Tepat Waktu",
        tepat_waktu,
        a.lunas,
        if a.lunas > 0 {
            (tepat_waktu as f64 / a.lunas as f64) * 100.0
        } else {
            0.0
        }
    );
    println!();

    if grand > 0.0 {
        println!("{}", style.bold("▌ KOMPOSISI PEMBAYARAN (SUDAH DIBAYAR)"));
        for (label, v) in [
            ("Pokok", total_pokok),
            ("Bunga", total_bunga),
            ("Admin", total_admin),
        ] {
            println!(
                "  {:<6} {} {:>14}  {:>5.1}%",
                label,
                mini_bar(pct_of(v, grand), 22),
                rupiah(v),
                pct_of(v, grand),
            );
        }
        println!("  {}", style.dim(&"─".repeat(52)));
        println!(
            "  {:<6} {:>40}  {:>5.1}%",
            style.bold("TOTAL"),
            style.bold(&rupiah(grand)),
            100.0
        );
        println!();
    }

    if a.belum_bayar > 0 {
        println!("{}", style.bold("▌ PROYEKSI"));
        println!("  {:<20} {} bulan", "Sisa angsuran", a.belum_bayar);
        println!("  {:<20} {}", "Estimasi lunas", terakhir);
        println!("  {:<20} {}", "Sisa tagihan", rupiah(r.total_hutang));
        println!("  {:<20} {}", "Per bulan (avg)", rupiah(angsuran_per_bulan));
        if let Some(jt) = &a.jatuh_tempo_terdekat {
            println!("  {:<20} {}", "Jatuh tempo next", style.bold(jt));
        }
        println!();
    }

    println!(
        "{}  {} baris, {} lunas, {} belum",
        style.bold("▌ TABEL ANGSURAN"),
        a.total_angsuran,
        style.green(&a.lunas.to_string()),
        if a.belum_bayar > 0 {
            style.yellow(&a.belum_bayar.to_string())
        } else {
            a.belum_bayar.to_string()
        }
    );
    println!();
    println!(
        "{:>3}  {:<11}  {:<9}  {:>12}  {:>12}  {:>12}  {:>12}  {:>6}",
        "No", "JatuhTempo", "Status", "Pokok", "Bunga", "Admin", "Total", "Telat"
    );
    println!("  {}", style.dim(&"─".repeat(93)));
    for b in &r.tabel_angsuran {
        let status_colored = match b.status.as_str() {
            "LUNAS" => style.green(&b.status),
            "-" => style.dim(&b.status),
            s if s.contains("TELAT") || s.contains("OVERDUE") => style.red(&b.status),
            s => style.yellow(s),
        };
        let telat = if b.hari_telat > 0 {
            style.red(&format!("{} hr", b.hari_telat))
        } else {
            style.dim(&format!("{} hr", b.hari_telat))
        };
        println!(
            "{:>3}  {:<11}  {:<9}  {:>12}  {:>12}  {:>12}  {:>12}  {:>6}",
            b.no,
            b.tgl_jatuh_tempo,
            status_colored,
            b.j_pokok,
            b.j_bunga,
            b.j_adm,
            style.bold(&b.j_total),
            telat,
        );
    }
    println!();

    if !lunas_rows.is_empty() {
        println!("{}", style.bold("▌ RIWAYAT PEMBAYARAN (LUNAS)"));
        for b in &lunas_rows {
            let tgl = if b.tgl_lunas_full.is_empty() {
                &b.tgl_jatuh_tempo
            } else {
                &b.tgl_lunas_full
            };
            let badge = if b.hari_telat == 0 {
                style.green("tepat waktu")
            } else {
                style.yellow(&format!("telat {} hr", b.hari_telat))
            };
            println!("  #{:<3} {}  {:>12}  {}", b.no, tgl, b.j_total, badge);
        }
        println!();
    }

    println!("{}", style.bold("▌ KESIMPULAN & REKOMENDASI"));
    match a.status {
        StatusKesehatan::Sehat => {
            println!("  {} Semua pembayaran lancar.", style.green("✓"));
            println!(
                "  {} {}/{} angsuran sudah lunas ({:.1}%).",
                style.dim("→"),
                a.lunas,
                a.total_angsuran,
                a.persentase_progress
            );
            if let Some(jt) = &a.jatuh_tempo_terdekat {
                println!(
                    "  {} Jatuh tempo berikutnya: {}",
                    style.dim("→"),
                    style.bold(jt)
                );
            }
            println!(
                "  {} Tidak perlu tindakan khusus, lanjutkan monitoring rutin.",
                style.dim("→")
            );
        }
        StatusKesehatan::Perhatian => {
            println!(
                "  {} Ada {} angsuran telat — perlu perhatian.",
                style.yellow("⚠"),
                a.telat
            );
            if let Some(jt) = &a.jatuh_tempo_terdekat {
                println!(
                    "  {} Jatuh tempo terdekat: {}",
                    style.dim("→"),
                    style.bold(jt)
                );
            }
            println!(
                "  {} Segera hubungi nasabah untuk konfirmasi jadwal bayar.",
                style.dim("→")
            );
        }
        StatusKesehatan::Bahaya => {
            println!(
                "  {} Ada {} angsuran telat — PRIORITAS COLLECTION.",
                style.red("✗"),
                a.telat
            );
            println!(
                "  {} Total tunggakan aktif: {}",
                style.dim("→"),
                style.bold(&rupiah(r.total_hutang))
            );
            println!(
                "  {} Eskalasi ke atasan PIC & jadwalkan kunjungan lapangan.",
                style.dim("→")
            );
        }
    }
    println!();
    println!("{}", style.bold(&sep));
    println!();

    Ok(())
}

fn render_generic(
    style: &Style,
    title: &str,
    v: &serde_json::Value,
    json_mode: bool,
    csv_mode: bool,
) -> Result<(), JssError> {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(v).unwrap());
        return Ok(());
    }

    let message = v.get("message").unwrap_or(v);

    if csv_mode {
        csv_from_json(message)?;
        return Ok(());
    }

    println!("{}", style.bold(title));
    println!();

    match message {
        serde_json::Value::Array(arr) => {
            println!("  {} items", style.cyan(&arr.len().to_string()));
            println!();
            for (i, item) in arr.iter().take(20).enumerate() {
                println!("  [{}] {}", i + 1, compact(item));
            }
            if arr.len() > 20 {
                println!("  ... dan {} lagi", arr.len() - 20);
                println!();
                println!("  {}", style.dim("gunakan --json untuk lihat semua"));
            }
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                let val = &map[k];
                println!("  {:<24} {}", style.cyan(k), compact(val));
            }
        }
        other => {
            println!("  {}", compact(other));
        }
    }

    Ok(())
}

fn csv_from_json(v: &serde_json::Value) -> Result<(), JssError> {
    let arr = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(o) if o.contains_key("data") => {
            if let Some(a) = o["data"].as_array() {
                a
            } else {
                return Err(JssError::Validation("data bukan array".into()));
            }
        }
        _ => {
            return Err(JssError::Validation(
                "struktur response tidak dikenal untuk CSV".into(),
            ));
        }
    };

    if arr.is_empty() {
        return Ok(());
    }

    let first = match arr[0].as_object() {
        Some(o) => o,
        None => return Err(JssError::Validation("item bukan object".into())),
    };

    let headers: Vec<&String> = first.keys().collect();
    let mut out = std::io::stdout();
    writeln!(
        out,
        "{}",
        headers
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
    .ok();

    for item in arr {
        let row: Vec<String> = headers
            .iter()
            .map(|h| csv_escape(&json_to_cell(&item[h])))
            .collect();
        writeln!(out, "{}", row.join(",")).ok();
    }

    Ok(())
}

fn json_to_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => v.to_string(),
    }
}

fn compact(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            if s.chars().count() > 60 {
                let t: String = s.chars().take(57).collect();
                format!("\"{}…\"", t)
            } else {
                format!("\"{}\"", s)
            }
        }
        serde_json::Value::Array(a) => format!("[{} items]", a.len()),
        serde_json::Value::Object(o) => {
            let mut s = String::from("{");
            for (i, (k, v2)) in o.iter().take(4).enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(k);
                s.push('=');
                s.push_str(&compact(v2));
            }
            if o.len() > 4 {
                s.push_str(&format!(", …+{}", o.len() - 4));
            }
            s.push('}');
            s
        }
        other => other.to_string(),
    }
}
