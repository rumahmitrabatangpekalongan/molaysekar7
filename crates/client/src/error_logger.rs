use chrono::Utc;
use shared::GlobalErrorLog;
use std::sync::RwLock;
use std::time::Duration;

pub const GLOBAL_ERROR_ENDPOINT: &str = "https://molay.sekolahkartini7.web.id/logerorglobal";
pub const CLIENT_VERSION: &str = "1.0.1";
pub const CLIENT_ERROR_LOG_FILE: &str = "client_error.log";

static FALLBACK_ENDPOINTS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Mendaftarkan endpoint server cadangan (misal LAN server IP http://192.168.x.x:1916)
pub fn register_fallback_endpoint(url: String) {
    let clean_url = url.trim_end_matches('/').to_string();
    if clean_url.is_empty() {
        return;
    }
    if let Ok(mut list) = FALLBACK_ENDPOINTS.write() {
        if !list.contains(&clean_url) {
            list.push(clean_url);
        }
    }
}

/// Menyimpan entri error ke file log lokal di komputer klien (client_error.log)
pub fn append_local_error(entry: &GlobalErrorLog) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(CLIENT_ERROR_LOG_FILE)
    {
        let line = format!(
            "[{}] [{}] Client: {:?}, Host: {:?} | {} | Details: {:?}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
            entry.error_type,
            entry.client_id,
            entry.hostname,
            entry.message,
            entry.details
        );
        let _ = file.write_all(line.as_bytes());
    }
}

/// Mengirimkan laporan log error ke server global secara asinkron tanpa memblokir jalannya program
pub fn send_global_error(
    error_type: impl Into<String>,
    message: impl Into<String>,
    details: Option<String>,
    client_id: Option<String>,
) {
    let err_type = error_type.into();
    let msg = message.into();
    eprintln!("[ERROR-LOG] [{}] {}: {}", Utc::now().format("%H:%M:%S"), err_type, msg);

    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok();

    let entry = GlobalErrorLog {
        id: format!("err_{}", Utc::now().timestamp_millis()),
        timestamp: Utc::now(),
        source: "client".to_string(),
        client_id,
        hostname,
        ip: None,
        error_type: err_type,
        message: msg,
        details,
        app_version: Some(CLIENT_VERSION.to_string()),
    };

    // 1. Simpan selalu ke log file lokal di PC Klien
    append_local_error(&entry);

    // Ambil daftar endpoint server cadangan (LAN) yang terdaftar
    let fallbacks: Vec<String> = if let Ok(guard) = FALLBACK_ENDPOINTS.read() {
        guard.clone()
    } else {
        Vec::new()
    };

    // 2. Kirim asinkron ke server global (Cloudflare) dan fallback server (LAN)
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };

        // Coba kirim ke endpoint utama Cloudflare
        let primary_res = client
            .post(GLOBAL_ERROR_ENDPOINT)
            .json(&entry)
            .send()
            .await;

        // Jika endpoint cloud gagal/tidak terjangkau (misal internet mati atau LAN-only),
        // coba kirimkan langsung ke Server Guru via LAN (http://<server-ip>:1916/logerorglobal)
        if primary_res.is_err() {
            for fb in fallbacks {
                let target_url = format!("{}/logerorglobal", fb.trim_end_matches('/'));
                let _ = client.post(&target_url).json(&entry).send().await;
            }
        }
    });
}

/// Menangkap panic runtime dan melaporkannya sebelum program crash
pub fn init_panic_hook(client_id: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let details = format!("Panic di {}: {}", location, payload);
        eprintln!("[FATAL PANIC] {}", details);

        // Kirim sinkron dengan timeout pendek sebelum keluar
        let entry = GlobalErrorLog {
            id: format!("panic_{}", Utc::now().timestamp_millis()),
            timestamp: Utc::now(),
            source: "client".to_string(),
            client_id: Some(client_id.clone()),
            hostname: std::env::var("COMPUTERNAME").ok(),
            ip: None,
            error_type: "FatalPanic".to_string(),
            message: payload,
            details: Some(details),
            app_version: Some(CLIENT_VERSION.to_string()),
        };

        // Buat runtime terpisah singkat untuk blocking call
        if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
            let _ = rt.block_on(async {
                if let Ok(c) = reqwest::Client::builder().timeout(Duration::from_secs(3)).build() {
                    let _ = c.post(GLOBAL_ERROR_ENDPOINT).json(&entry).send().await;
                }
            });
        }

        default_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_fallback_endpoint() {
        register_fallback_endpoint("http://192.168.1.10:1916/".to_string());
        let list = FALLBACK_ENDPOINTS.read().unwrap();
        assert!(list.contains(&"http://192.168.1.10:1916".to_string()));
    }

    #[test]
    fn test_append_local_error() {
        let entry = GlobalErrorLog {
            id: "test_err_1".to_string(),
            timestamp: Utc::now(),
            source: "client".to_string(),
            client_id: Some("PC-TEST".to_string()),
            hostname: Some("PC-TEST-HOST".to_string()),
            ip: None,
            error_type: "TestError".to_string(),
            message: "Simulated test error".to_string(),
            details: Some("Detail string".to_string()),
            app_version: Some("1.0.1".to_string()),
        };
        append_local_error(&entry);
        assert!(std::path::Path::new(CLIENT_ERROR_LOG_FILE).exists());
        let _ = std::fs::remove_file(CLIENT_ERROR_LOG_FILE);
    }
}
