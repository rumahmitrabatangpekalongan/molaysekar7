use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use shared::UpdateInfo;
use std::time::Duration;

pub const CURRENT_VERSION: &str = "1.0.4";
pub const DEFAULT_UPDATE_API_KEY: &str = "cfut_EFE4TDO6DKe6tGvVKfgGgjW1Hldzn6ss43VLVg2J7b17463a";
pub const UPDATE_KEY_FILE: &str = "update_key.txt";

/// Memuat API Key dari file `update_key.txt` atau memakai default Cloudflare Bearer token
pub fn get_api_key() -> String {
    if let Ok(key) = std::fs::read_to_string(UPDATE_KEY_FILE) {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    DEFAULT_UPDATE_API_KEY.to_string()
}

/// Bersihkan file temporary .old sisa update sebelumnya
pub fn cleanup_old_binary() {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("MolayClient-Sekar7") && (name.contains(".old") || name.ends_with(".new")) {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                }
            }
        }
    }
}

/// Membuat reqwest client dengan header Authorization Bearer yang aman
pub fn create_http_client(api_key: &str) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    let auth_val = format!("Bearer {}", api_key.trim());
    if let Ok(val) = HeaderValue::from_str(&auth_val) {
        headers.insert(AUTHORIZATION, val);
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Gagal inisialisasi HTTP client: {}", e))
}

/// Memeriksa pembaruan ke server
pub async fn check_for_update(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Option<UpdateInfo>, String> {
    let clean_base = base_url.trim_end_matches('/');
    let url = format!("{}/api/update/check?current_version={}", clean_base, CURRENT_VERSION);

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Gagal menghubungi server update di {}: {}", url, e))?;

    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Akses update ditolak: API Key / Bearer token tidak valid!".to_string());
    }

    if !res.status().is_success() {
        return Err(format!("Server update mengembalikan status error: {}", res.status()));
    }

    let info: UpdateInfo = res
        .json()
        .await
        .map_err(|e| format!("Format respons update tidak valid: {}", e))?;

    if info.update_available {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Mengunduh binary baru dan menggantikan executable saat ini (Self-Update di Windows)
pub async fn download_and_self_update(
    client: &reqwest::Client,
    base_url: &str,
    info: &UpdateInfo,
) -> Result<(), String> {
    let clean_base = base_url.trim_end_matches('/');
    let download_url = if info.download_url.starts_with("http://") || info.download_url.starts_with("https://") {
        info.download_url.clone()
    } else {
        format!("{}{}", clean_base, info.download_url)
    };

    println!("[UPDATER] Mengunduh binary baru dari {}...", download_url);

    let res = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Gagal mengunduh pembaruan: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Gagal unduh binary: HTTP {}", res.status()));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Gagal membaca data binary unduhan: {}", e))?;

    if bytes.len() < 100_000 {
        return Err(format!("Ukuran file unduhan terlalu kecil ({} bytes), kemungkinan corrupt atau bukan exe.", bytes.len()));
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Gagal menentukan path executable saat ini: {}", e))?;

    let temp_dir = std::env::temp_dir();
    let preferred_new_exe = current_exe.with_extension("exe.new");
    let fallback_new_exe = temp_dir.join("MolayClient-Sekar7.exe.new");
    let old_exe = current_exe.with_extension("exe.old");

    // Simpan data unduhan: coba simpan langsung di folder app, jika gagal coba simpan di %TEMP%
    let new_exe = if std::fs::write(&preferred_new_exe, &bytes).is_ok() {
        preferred_new_exe
    } else {
        std::fs::write(&fallback_new_exe, &bytes)
            .map_err(|e| format!("Gagal menyimpan file unduhan baik di folder app maupun %TEMP%: {}", e))?;
        fallback_new_exe
    };

    println!("[UPDATER] Berhasil mengunduh {} bytes. Menerapkan pembaruan...", bytes.len());

    // Di Windows: Rename file berjalan (.exe -> .old), lalu ganti dengan (.new -> .exe)
    if old_exe.exists() {
        let _ = std::fs::remove_file(&old_exe);
    }

    let mut direct_replaced = false;
    if std::fs::rename(&current_exe, &old_exe).is_ok() {
        if std::fs::rename(&new_exe, &current_exe).is_ok() {
            direct_replaced = true;
        } else if std::fs::copy(&new_exe, &current_exe).is_ok() {
            let _ = std::fs::remove_file(&new_exe);
            direct_replaced = true;
        } else {
            // Rollback jika gagal
            let _ = std::fs::rename(&old_exe, &current_exe);
        }
    }

    if !direct_replaced {
        eprintln!("[UPDATER WARNING] Penggantian langsung gagal. Mencoba elevated updater...");

        // Jika rename langsung gagal (misal Access is denied / OS Error 5 di Program Files),
        // buat skrip elevated helper untuk menyelesaikan penggantian file dengan hak Administrator
        let batch_script = temp_dir.join("molay_apply_update.bat");
        let script_content = format!(
            "@echo off\r\n\
             timeout /t 1 /nobreak >nul\r\n\
             if exist \"{}\" del /f /q \"{}\" >nul 2>&1\r\n\
             move /y \"{}\" \"{}\" >nul 2>&1\r\n\
             move /y \"{}\" \"{}\" >nul 2>&1\r\n\
             start \"\" \"{}\"\r\n\
             del \"%~f0\"\r\n",
            old_exe.display(),
            old_exe.display(),
            current_exe.display(),
            old_exe.display(),
            new_exe.display(),
            current_exe.display(),
            current_exe.display(),
        );

        if std::fs::write(&batch_script, script_content).is_ok() {
            let spawn_res = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-WindowStyle", "Hidden",
                    "-Command",
                    &format!("Start-Process cmd -ArgumentList '/c \"{}\"' -Verb RunAs", batch_script.display())
                ])
                .spawn();

            if spawn_res.is_ok() {
                println!("[UPDATER] Helper elevated updater dijalankan. Keluar untuk menerapkan pembaruan...");
                std::process::exit(0);
            }
        }

        return Err(format!("Gagal memindahkan file aktif ke {:?}", old_exe));
    }

    println!("[UPDATER] Pembaruan berhasil diterapkan! Me-restart aplikasi ke versi baru...");

    // Jalankan executable baru
    let mut cmd = std::process::Command::new(&current_exe);
    cmd.args(std::env::args().skip(1));
    if let Err(e) = cmd.spawn() {
        return Err(format!("Pembaruan terpasang tetapi gagal me-restart otomatis: {}", e));
    }

    // Keluar dari proses saat ini agar proses baru mengambil alih
    std::process::exit(0);
}
