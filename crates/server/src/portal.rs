use axum::{
    extract::{Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Handler utama untuk rute `/`
/// Secara cerdas membedakan:
/// - Jika diakses lewat `molay.sekolahkartini7.web.id` -> Menyajikan Halaman Portal Identitas & Informasi (`portal.html`)
/// - Jika diakses lewat `monitoringmolay.sekolahkartini7.web.id`, `localhost`, atau IP LAN -> Menyajikan Dashboard Monitoring Guru (`index.html`) dengan Login Overlay
/// - Jika memiliki query `?mode=admin` atau `?mode=dashboard` -> Menyajikan Dashboard Monitoring Guru
pub async fn index_dispatch_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Html<&'static str> {
    // 1. Periksa parameter eksplisit ?mode=admin / ?mode=dashboard
    if let Some(mode) = query.get("mode") {
        if mode == "admin" || mode == "dashboard" {
            return Html(include_str!("index.html"));
        }
    }

    // 2. Periksa Host Header
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // Domain publik identitas sekolah: molay.sekolahkartini7.web.id
    // (PENTING: Pastikan BUKAN subdomain monitoringmolay...)
    if host.contains("molay.sekolahkartini7.web.id") && !host.contains("monitoringmolay") {
        return Html(include_str!("portal.html"));
    }

    // Default untuk monitoringmolay.sekolahkartini7.web.id, localhost, 127.0.0.1, atau IP LAN:
    // Selalu menyajikan Dashboard Monitoring Guru & Admin yang terproteksi login
    Html(include_str!("index.html"))
}

/// Handler rute eksplisit `/portal`
pub async fn portal_page_handler() -> Html<&'static str> {
    Html(include_str!("portal.html"))
}

/// Handler rute eksplisit `/admin` dan `/dashboard`
pub async fn admin_page_handler() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

/// Handler unduhan aman file installer/dist publik (/dist/:filename dan /download/:filename)
pub async fn download_dist_file_handler(Path(filename): Path<String>) -> Response {
    let clean_name = filename.trim();

    // Whitelist file yang diizinkan untuk diunduh secara publik
    let allowed_files = [
        "MolayClient-Sekar7-Setup.exe",
        "MolayServer-Sekar7-Setup.exe",
        "Molay-Sekar7-Setup-Lengkap.exe",
        "MolayClient-Sekar7.exe",
        "MolayServer-Sekar7.exe",
    ];

    if !allowed_files.contains(&clean_name) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Akses ditolak: File tidak terdaftar dalam paket distribusi publik."
            })),
        )
            .into_response();
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut candidate_paths = vec![
        PathBuf::from("dist").join("Installers").join(clean_name),
        PathBuf::from("dist").join("Molay-Client").join(clean_name),
        PathBuf::from("dist").join("Molay-Server").join(clean_name),
        PathBuf::from("dist").join(clean_name),
        PathBuf::from("installers").join(clean_name),
        PathBuf::from(clean_name),
        PathBuf::from("..").join("dist").join("Installers").join(clean_name),
        PathBuf::from("..").join("dist").join("Molay-Client").join(clean_name),
        PathBuf::from("..").join("dist").join(clean_name),
        PathBuf::from("..").join("installers").join(clean_name),
    ];

    if let Some(ref dir) = exe_dir {
        candidate_paths.push(dir.join("dist").join("Installers").join(clean_name));
        candidate_paths.push(dir.join("dist").join("Molay-Client").join(clean_name));
        candidate_paths.push(dir.join("dist").join("Molay-Server").join(clean_name));
        candidate_paths.push(dir.join("dist").join(clean_name));
        candidate_paths.push(dir.join("installers").join(clean_name));
        candidate_paths.push(dir.join(clean_name));
        candidate_paths.push(dir.join("..").join("dist").join("Installers").join(clean_name));
        candidate_paths.push(dir.join("..").join("dist").join("Molay-Client").join(clean_name));
        candidate_paths.push(dir.join("..").join("installers").join(clean_name));
    }

    let mut found_path = None;
    for path in &candidate_paths {
        if path.exists() && path.is_file() {
            found_path = Some(path.clone());
            break;
        }
    }

    let path = match found_path {
        Some(p) => p,
        None => {
            eprintln!("[DIST DOWNLOAD 404] File '{}' tidak ditemukan di lokasi penyimpanan.", clean_name);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("File installer '{}' belum tersedia di server.", clean_name)
                })),
            )
                .into_response();
        }
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/vnd.microsoft.portable-executable"),
                (
                    header::CONTENT_DISPOSITION,
                    &format!("attachment; filename=\"{}\"", clean_name),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Gagal membaca file dari server: {}", e)
            })),
        )
            .into_response(),
    }
}
