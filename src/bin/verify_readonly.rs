use librjss::{ClientConfig, JssError, RjssClient};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, fs};

const DOCTYPES_TO_CHECK: &[&str] = &[
    "BSTK",
    "TTS",
    "Data Pengajuan",
    "Master Data Nasabah",
    "Target dan Realisasi",
    "Deviasi",
    "Surat Diskresi",
    "Request Dokumen Collection",
    "Contact",
    "User",
    "File",
    "ToDo",
];

#[derive(Serialize, Deserialize, Debug)]
struct Snapshot {
    taken_at: String,
    counts: BTreeMap<String, u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("snapshot") => {
            let out_file = args
                .get(2)
                .ok_or("pemakaian: verify_readonly snapshot <file.json>")?;
            let snap = take_snapshot().await?;
            fs::write(out_file, serde_json::to_string_pretty(&snap)?)?;
            println!("✓ Snapshot disimpan ke {}", out_file);
            for (dt, n) in &snap.counts {
                println!(
                    "  {:35} : {}",
                    dt,
                    if *n == u64::MAX {
                        "n/a".to_string()
                    } else {
                        n.to_string()
                    }
                );
            }
        }
        Some("compare") => {
            let before_file = args
                .get(2)
                .ok_or("pemakaian: verify_readonly compare <before.json> <after.json>")?;
            let after_file = args
                .get(3)
                .ok_or("pemakaian: verify_readonly compare <before.json> <after.json>")?;
            let before: Snapshot = serde_json::from_str(&fs::read_to_string(before_file)?)?;
            let after: Snapshot = serde_json::from_str(&fs::read_to_string(after_file)?)?;
            compare(&before, &after);
        }
        _ => {
            eprintln!("Pemakaian:");
            eprintln!("  verify_readonly snapshot <file.json>");
            eprintln!("  verify_readonly compare <before.json> <after.json>");
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn take_snapshot() -> Result<Snapshot, JssError> {
    let cfg = ClientConfig::from_env()?;

    let mut client = RjssClient::new(cfg)?;
    client.authenticate().await?;

    let mut counts = BTreeMap::new();
    for dt in DOCTYPES_TO_CHECK {
        let mut args: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        args.insert("doctype".into(), (*dt).into());

        let n = match client
            .call_method("frappe.client.get_count", Some(args))
            .await
        {
            Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["message"].as_u64())
                .unwrap_or(0),
            Err(_) => u64::MAX,
        };
        counts.insert((*dt).to_string(), n);
        println!(
            "  {:35} : {}",
            dt,
            if n == u64::MAX {
                "n/a".to_string()
            } else {
                n.to_string()
            }
        );
    }

    Ok(Snapshot {
        taken_at: chrono::Utc::now().to_rfc3339(),
        counts,
    })
}

fn compare(before: &Snapshot, after: &Snapshot) {
    println!("\n═══ PERBANDINGAN SNAPSHOT ═══");
    println!("  Before : {}", before.taken_at);
    println!("  After  : {}", after.taken_at);
    println!();

    let mut any_change = false;
    println!(
        "  {:35} {:>10} {:>10} {:>10}",
        "DocType", "Before", "After", "Delta"
    );
    println!("  {}", "─".repeat(70));

    for (dt, b) in &before.counts {
        let a = after.counts.get(dt).copied().unwrap_or(*b);
        let delta = if *b == u64::MAX || a == u64::MAX {
            "n/a".to_string()
        } else {
            format!("{:+}", a as i64 - *b as i64)
        };
        let flag = if delta.starts_with('+') || delta.starts_with('-') {
            any_change = true;
            " ⚠️"
        } else {
            ""
        };
        println!(
            "  {:35} {:>10} {:>10} {:>10}{}",
            dt,
            if *b == u64::MAX {
                "n/a".into()
            } else {
                b.to_string()
            },
            if a == u64::MAX {
                "n/a".into()
            } else {
                a.to_string()
            },
            delta,
            flag,
        );
    }

    println!();
    if any_change {
        println!("❌ ADA PERUBAHAN pada salah satu DocType. Investigasi!");
        println!("   Kemungkinan: user lain sedang bekerja, atau ada mutasi nyata.");
        std::process::exit(2);
    } else {
        println!("✅ TIDAK ADA PERUBAHAN. Server bersih.");
        println!("   Semua request dari client kita murni read-only.");
    }
}
