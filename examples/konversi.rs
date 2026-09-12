use librjss::{ClientConfig, JssError, RjssClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: konversi <id>");
        eprintln!();
        eprintln!("  <id> bisa berupa:");
        eprintln!("    no_perjanjian  (contoh: OD-26-6003-0000974-0001)");
        eprintln!("    register_id    (contoh: ODR-0726-00070)");
        std::process::exit(1);
    }

    let input = args[0].trim();
    let cfg = ClientConfig::from_env()?;
    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    let jenis = deteksi(input);
    println!("input      : {}", input);
    println!("terdeteksi : {}", jenis);
    println!();

    match jenis {
        Jenis::NoPerjanjian => {
            let Some(mdn) = ambil_mdn(&client, input).await? else {
                eprintln!(
                    "Master Data Nasabah dengan no_perjanjian '{}' tidak ditemukan",
                    input
                );
                std::process::exit(2);
            };
            print_mdn(&mdn);

            match mdn.get("register_id").and_then(|v| v.as_str()) {
                Some(reg) if !reg.is_empty() => {
                    println!();
                    println!("relasi -> register_id: {}", reg);
                    if let Some(dp) = ambil_dp(&client, reg).await? {
                        println!();
                        print_dp(&dp);
                    } else {
                        println!();
                        println!("(Data Pengajuan '{}' tidak ditemukan)", reg);
                    }
                }
                _ => {
                    println!();
                    println!("(Master Data Nasabah ini belum punya register_id)");
                }
            }
        }

        Jenis::RegisterId => {
            let Some(dp) = ambil_dp(&client, input).await? else {
                eprintln!("Data Pengajuan dengan name '{}' tidak ditemukan", input);
                std::process::exit(2);
            };
            print_dp(&dp);

            match dp.get("no_perjanjian").and_then(|v| v.as_str()) {
                Some(np) if !np.is_empty() => {
                    println!();
                    println!("relasi -> no_perjanjian: {}", np);
                    if let Some(mdn) = ambil_mdn(&client, np).await? {
                        println!();
                        print_mdn(&mdn);
                    } else {
                        println!();
                        println!("(Master Data Nasabah '{}' belum dibuat)", np);
                    }
                }
                _ => {
                    println!();
                    println!("(Data Pengajuan ini belum punya no_perjanjian — belum pencairan)");
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum Jenis {
    NoPerjanjian,
    RegisterId,
}

impl std::fmt::Display for Jenis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Jenis::NoPerjanjian => f.write_str("no_perjanjian (Master Data Nasabah)"),
            Jenis::RegisterId => f.write_str("register_id (Data Pengajuan)"),
        }
    }
}

fn deteksi(s: &str) -> Jenis {
    if s.starts_with("ODR-") {
        Jenis::RegisterId
    } else {
        Jenis::NoPerjanjian
    }
}

async fn ambil_mdn(
    client: &RjssClient,
    no_perjanjian: &str,
) -> Result<Option<serde_json::Value>, JssError> {
    ambil_doc(client, "Master Data Nasabah", no_perjanjian).await
}

async fn ambil_dp(
    client: &RjssClient,
    register_id: &str,
) -> Result<Option<serde_json::Value>, JssError> {
    ambil_doc(client, "Data Pengajuan", register_id).await
}

async fn ambil_doc(
    client: &RjssClient,
    doctype: &str,
    name: &str,
) -> Result<Option<serde_json::Value>, JssError> {
    let path = format!(
        "/api/resource/{}/{}",
        urlencoding::encode(doctype),
        urlencoding::encode(name)
    );

    match client.authenticated_get(&path).await {
        Ok(raw) => {
            let v: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| JssError::Parse(e.to_string()))?;
            Ok(v.get("data").cloned())
        }
        Err(e) if is_404(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

fn is_404(e: &JssError) -> bool {
    match e {
        JssError::Http { status, .. } => status.as_u16() == 404,
        JssError::ApiError { status, .. } => status.as_u16() == 404,
        _ => false,
    }
}

fn print_mdn(v: &serde_json::Value) {
    println!("Master Data Nasabah");
    println!("  name              : {}", s(v, "name"));
    println!("  register_id       : {}", s(v, "register_id"));
    println!("  nama_nasabah      : {}", s(v, "nama_nasabah"));
    println!("  no_perjanjian     : {}", s(v, "no_perjanjian"));
    println!("  no_rekening_kredit: {}", s(v, "no_rekening_kredit"));
    println!("  status            : {}", s(v, "status"));
    println!("  status_piutang    : {}", s(v, "status_piutang"));
    println!("  status_bpkb       : {}", s(v, "status_bpkb"));
    println!("  cabang            : {}", s(v, "cabang"));
    println!("  no_hp_1           : {}", s(v, "no_hp_1"));
    println!("  nopol             : {}", s(v, "nopol"));
    println!(
        "  kendaraan         : {} {} {} ({})",
        s(v, "merk"),
        s(v, "model"),
        s(v, "tahun"),
        s(v, "warna")
    );
    println!("  pinjaman          : {}", f(v, "pinjaman"));
    println!("  angsuran          : {}", f(v, "angsuran"));
    println!("  tenor             : {}", s(v, "tenor"));
    println!("  tanggal_pencairan : {}", s(v, "tanggal_pencairan"));
    println!("  tanggal_jto_1     : {}", s(v, "tanggal_jto_1"));
}

fn print_dp(v: &serde_json::Value) {
    println!("Data Pengajuan");
    println!("  name              : {}", s(v, "name"));
    println!("  no_perjanjian     : {}", s(v, "no_perjanjian"));
    println!("  nama_lengkap      : {}", s(v, "nama_lengkap"));
    println!("  nik               : {}", s(v, "nik"));
    println!("  no_hp             : {}", s(v, "no_hp"));
    println!("  status            : {}", s(v, "status"));
    println!("  cabang            : {}", s(v, "cabang"));
    println!("  marketing         : {}", s(v, "marketing"));
    println!("  marketing_full_name: {}", s(v, "marketing_full_name"));
    println!("  pinjam            : {}", f(v, "pinjam"));
    println!("  angsuran          : {}", f(v, "angsuran"));
    println!("  tenor             : {}", s(v, "tenor"));
    println!("  tanggal_pengajuan : {}", s(v, "tanggal_pengajuan"));
    println!("  tanggal_pencairan : {}", s(v, "tanggal_pencairan"));
    println!("  kategori          : {}", s(v, "kategori"));
    println!("  rekomendasi       : {}", s(v, "rekomendasi_marketing"));
}

fn s(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .map(String::from)
        .unwrap_or_else(|| "-".into())
}

fn f(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_f64())
        .map(|n| format!("{n:.0}"))
        .unwrap_or_else(|| "-".into())
}
