use librjss::{ClientConfig, JssError, RjssClient};
use std::path::PathBuf;

const FIELD_FILE: &[&str] = &[
    "ktp",
    "ktp_2",
    "ktp_3",
    "slip_gaji",
    "foto_tempat_tinggal",
    "vc_saat_kerja",
    "foto_rusak",
    "dokumen_stpl",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: download_kredit <register_id> [output_dir]");
        eprintln!();
        eprintln!("contoh:");
        eprintln!("  download_kredit ODR-0726-00070");
        eprintln!("  download_kredit ODR-0726-00070 ./kredit-files");
        std::process::exit(1);
    }

    let register_id = args[0].clone();
    let out_dir: PathBuf = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let cfg = ClientConfig::from_env()?;
    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    println!("register_id : {}", register_id);
    println!("output_dir  : {}", out_dir.display());
    println!();

    let doc = ambil_pengajuan(&client, &register_id).await?;

    std::fs::create_dir_all(&out_dir)
        .map_err(|e| JssError::FileOperation(format!("mkdir gagal: {e}")))?;

    let mut berhasil = 0u32;
    let mut gagal = 0u32;
    let mut dilewati = 0u32;

    for field in FIELD_FILE {
        let url = match doc.get(*field).and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => {
                dilewati += 1;
                continue;
            }
        };

        if !url.starts_with("/private/files/") && !url.starts_with("/files/") {
            dilewati += 1;
            continue;
        }

        let nama = nama_file(url)?;
        let tujuan = out_dir.join(&nama);

        if tujuan.exists() {
            println!("[skip] {} sudah ada: {}", field, tujuan.display());
            dilewati += 1;
            continue;
        }

        print!("[dl]  {} -> {}", field, tujuan.display());
        match client.download_file_to_path(url, &tujuan).await {
            Ok(()) => {
                let ukuran = std::fs::metadata(&tujuan).map(|m| m.len()).unwrap_or(0);
                println!("  ok ({} bytes)", ukuran);
                berhasil += 1;
            }
            Err(e) => {
                println!("  gagal: {}", e);
                gagal += 1;
            }
        }
    }

    println!();
    println!("berhasil: {}", berhasil);
    println!("gagal   : {}", gagal);
    println!("dilewati: {}", dilewati);

    Ok(())
}

async fn ambil_pengajuan(
    client: &RjssClient,
    register_id: &str,
) -> Result<serde_json::Value, JssError> {
    let path = format!(
        "/api/resource/{}/{}",
        urlencoding::encode("Data Pengajuan"),
        urlencoding::encode(register_id)
    );

    let raw = client.authenticated_get(&path).await?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| JssError::Parse(e.to_string()))?;

    v.get("data")
        .cloned()
        .ok_or_else(|| JssError::Parse("response tidak punya field data".into()))
}

fn nama_file(url: &str) -> Result<String, JssError> {
    let tanpa_query = url.split('?').next().unwrap_or(url);
    let bagian_akhir = tanpa_query
        .rsplit('/')
        .next()
        .ok_or_else(|| JssError::Validation(format!("url tidak valid: {url}")))?;

    let terdecode = urlencoding::decode(bagian_akhir)
        .map_err(|e| JssError::Validation(format!("decode url gagal: {e}")))?
        .into_owned();

    let bersih: String = terdecode
        .chars()
        .map(|c| if "\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();

    if bersih.is_empty() || bersih == "." || bersih == ".." {
        return Err(JssError::Validation(format!(
            "nama file tidak valid: {url}"
        )));
    }

    Ok(bersih)
}
