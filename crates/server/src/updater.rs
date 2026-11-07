use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use shared::{ServerMessage, UpdateInfo};
use std::path::PathBuf;
use std::time::Duration;

use crate::AppState;

pub const DEFAULT_UPDATE_API_KEY: &str = "cfut_EFE4TDO6DKe6tGvVKfgGgjW1Hldzn6ss43VLVg2J7b17463a";
pub const UPDATE_CONFIG_FILE: &str = "update_config.json";

fn default_cloud_manifest_url() -> String {
    "https://molay.sekolahkartini7.web.id/api/cloud-manifest.json".to_string()
}

fn default_auto_check() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub latest_version: String,
    pub release_notes: String,
    pub api_key: String,
    pub mandatory: bool,
    pub client_binary_path: String,
    #[serde(default = "default_cloud_manifest_url")]
    pub cloud_manifest_url: String,
    #[serde(default = "default_auto_check")]
    pub auto_check_cloud: bool,
    #[serde(default)]
    pub auto_sync_client: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            latest_version: "1.0.4".to_string(),
            release_notes: "Dukungan IP Server Baru Otomatis (Port 1916 default), Tampilan Dashboard LAN Dinamis, dan Hot-Reload Alamat Server.".to_string(),
            api_key: DEFAULT_UPDATE_API_KEY.to_string(),
            mandatory: false,
            client_binary_path: "dist/Molay-Client/MolayClient-Sekar7.exe".to_string(),
            cloud_manifest_url: default_cloud_manifest_url(),
            auto_check_cloud: true,
            auto_sync_client: false,
        }
    }
}

pub fn load_or_init_update_config() -> UpdateConfig {
    if let Ok(data) = std::fs::read_to_string(UPDATE_CONFIG_FILE) {
        if let Ok(cfg) = serde_json::from_str::<UpdateConfig>(&data) {
            println!("[UPDATER] Memuat konfigurasi update (Target Versi: v{})", cfg.latest_version);
            return cfg;
        }
    }

    let cfg = UpdateConfig::default();
    let _ = save_update_config(&cfg);
    println!("[UPDATER] Inisialisasi konfigurasi default update di {}", UPDATE_CONFIG_FILE);
    cfg
}

pub fn save_update_config(cfg: &UpdateConfig) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(UPDATE_CONFIG_FILE, json)
}

pub fn is_authorized(headers: &HeaderMap, configured_key: &str) -> bool {
    let configured = configured_key.trim();
    if configured.is_empty() {
        return true;
    }

    if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let trimmed = auth.trim();
        if let Some(token) = trimmed.strip_prefix("Bearer ") {
            let t = token.trim();
            if t == configured || t == DEFAULT_UPDATE_API_KEY {
                return true;
            }
        } else if trimmed == configured || trimmed == DEFAULT_UPDATE_API_KEY {
            return true;
        }
    }

    if let Some(custom) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let c = custom.trim();
        if c == configured || c == DEFAULT_UPDATE_API_KEY {
            return true;
        }
    }

    false
}

#[derive(Deserialize)]
pub struct CheckUpdateQuery {
    pub current_version: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn check_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CheckUpdateQuery>,
) -> Response {
    let cfg = state.update_config.read().await;

    if !is_authorized(&headers, &cfg.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized: Invalid or missing API key. Gunakan 'Authorization: Bearer <API_KEY>'".to_string(),
            }),
        )
            .into_response();
    }

    let current = query.current_version.unwrap_or_else(|| "1.0.0".to_string());
    let update_available = is_newer_version(&cfg.latest_version, &current);

    let info = UpdateInfo {
        update_available,
        current_version: current,
        latest_version: cfg.latest_version.clone(),
        release_notes: cfg.release_notes.clone(),
        download_url: "/api/update/client/download".to_string(),
        sha256: None,
        mandatory: cfg.mandatory,
    };

    (StatusCode::OK, Json(info)).into_response()
}

pub async fn download_client(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let cfg = state.update_config.read().await;

    if !is_authorized(&headers, &cfg.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized: Invalid or missing API key. Akses download ditolak.".to_string(),
            }),
        )
            .into_response();
    }

    // Cari path executable client dari berbagai kemungkinan lokasi
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut candidate_paths = vec![
        PathBuf::from(&cfg.client_binary_path),
        PathBuf::from("MolayClient-Sekar7.exe"),
        PathBuf::from("updates/MolayClient-Sekar7.exe"),
        PathBuf::from("Client/MolayClient-Sekar7.exe"),
        PathBuf::from("../Client/MolayClient-Sekar7.exe"),
        PathBuf::from("../Molay-Client/MolayClient-Sekar7.exe"),
        PathBuf::from("dist/Molay-Client/MolayClient-Sekar7.exe"),
        PathBuf::from("../dist/Molay-Client/MolayClient-Sekar7.exe"),
        PathBuf::from("target/release/MolayClient-Sekar7.exe"),
        PathBuf::from("../target/release/MolayClient-Sekar7.exe"),
    ];

    if let Some(ref dir) = exe_dir {
        candidate_paths.push(dir.join(&cfg.client_binary_path));
        candidate_paths.push(dir.join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("updates").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("Client").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("..").join("Client").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("..").join("Molay-Client").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("dist").join("Molay-Client").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("..").join("dist").join("Molay-Client").join("MolayClient-Sekar7.exe"));
        candidate_paths.push(dir.join("..").join("target").join("release").join("MolayClient-Sekar7.exe"));
    }

    let mut found_path = None;
    for path in &candidate_paths {
        if path.exists() && path.is_file() {
            println!("[UPDATER] Melayani binary pembaruan klien dari: {:?}", path);
            found_path = Some(path.clone());
            break;
        }
    }

    let path = match found_path {
        Some(p) => p,
        None => {
            eprintln!(
                "[UPDATER 404] File binary client tidak ditemukan! Diperiksa di {} kemungkinan lokasi.",
                candidate_paths.len()
            );
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "File binary client tidak ditemukan di path: {}",
                        cfg.client_binary_path
                    ),
                }),
            )
                .into_response();
        }
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"MolayClient-Sekar7.exe\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Gagal membaca file binary: {}", e),
            }),
        )
            .into_response(),
    }
}

pub async fn get_update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
) -> Response {
    let cfg = state.update_config.read().await;
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &cfg.api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }
    Json(cfg.clone()).into_response()
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub latest_version: String,
    pub release_notes: String,
    pub api_key: String,
    pub mandatory: Option<bool>,
    pub client_binary_path: Option<String>,
    pub cloud_manifest_url: Option<String>,
    pub auto_check_cloud: Option<bool>,
    pub auto_sync_client: Option<bool>,
}

pub async fn save_update_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
    Json(payload): Json<UpdateConfigPayload>,
) -> Response {
    let current_api_key = state.update_config.read().await.api_key.clone();
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &current_api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }

    let mut cfg = state.update_config.write().await;
    cfg.latest_version = payload.latest_version;
    cfg.release_notes = payload.release_notes;
    cfg.api_key = payload.api_key;
    if let Some(m) = payload.mandatory {
        cfg.mandatory = m;
    }
    if let Some(p) = payload.client_binary_path {
        cfg.client_binary_path = p;
    }
    if let Some(c) = payload.cloud_manifest_url {
        cfg.cloud_manifest_url = c;
    }
    if let Some(a) = payload.auto_check_cloud {
        cfg.auto_check_cloud = a;
    }
    if let Some(s) = payload.auto_sync_client {
        cfg.auto_sync_client = s;
    }

    let _ = save_update_config(&cfg);
    println!("[UPDATER] Konfigurasi update berhasil diperbarui oleh admin (Target Versi: v{})", cfg.latest_version);
    Json(cfg.clone()).into_response()
}

pub async fn broadcast_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
) -> Response {
    let cfg = state.update_config.read().await;
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &cfg.api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }

    let info = UpdateInfo {
        update_available: true,
        current_version: "current".to_string(),
        latest_version: cfg.latest_version.clone(),
        release_notes: cfg.release_notes.clone(),
        download_url: "/api/update/client/download".to_string(),
        sha256: None,
        mandatory: cfg.mandatory,
    };

    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for tx in senders.values() {
        let msg = ServerMessage::UpdateNotification { info: info.clone() };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        }
    }

    println!("[UPDATER] Siaran update v{} terkirim ke {} klien online", cfg.latest_version, sent_count);

    Json(serde_json::json!({
        "success": true,
        "sent_count": sent_count,
        "target_version": cfg.latest_version,
    }))
    .into_response()
}

/// Bersihkan sisa temporary .old/.new dari update server sebelumnya
pub fn cleanup_old_server_binary() {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("MolayServer-Sekar7") && (name.contains(".old") || name.ends_with(".new")) {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudManifest {
    pub server_version: String,
    #[serde(default)]
    pub client_version: Option<String>,
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub server_download_url: Option<String>,
    #[serde(default)]
    pub client_download_url: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
}

pub async fn fetch_manifest_with_auth(
    manifest_url: &str,
    api_key: Option<&str>,
) -> Result<CloudManifest, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| format!("HTTP Client error: {}", e))?;

    let effective_key = api_key
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .unwrap_or(DEFAULT_UPDATE_API_KEY);

    let mut req = client.get(manifest_url);
    if !effective_key.is_empty() {
        req = req.header(header::AUTHORIZATION, format!("Bearer {}", effective_key));
    }

    let direct_res = req.send().await;

    // 1. Coba baca format CloudManifest langsung jika endpoint manifest mengembalikan respon valid
    if let Ok(res) = direct_res {
        if res.status().is_success() {
            if let Ok(manifest) = res.json::<CloudManifest>().await {
                return Ok(manifest);
            }
        }
    }

    // 2. Fallback cerdas: Jika /api/cloud-manifest.json 404 atau bukan format manifest langsung,
    // coba periksa endpoint update server standar di origin host (/api/update/config & /api/update/check)
    if let Ok(parsed_url) = reqwest::Url::parse(manifest_url) {
        let origin = parsed_url.origin().ascii_serialization();

        // Panggil /api/update/config di Cloud Server
        let config_url = format!("{}/api/update/config", origin);
        let mut cfg_req = client.get(&config_url);
        if !effective_key.is_empty() {
            cfg_req = cfg_req.header(header::AUTHORIZATION, format!("Bearer {}", effective_key));
        }

        if let Ok(res) = cfg_req.send().await {
            if res.status().is_success() {
                #[derive(Deserialize)]
                struct CloudConfigResp {
                    latest_version: String,
                    #[serde(default)]
                    release_notes: Option<String>,
                }

                if let Ok(cfg_data) = res.json::<CloudConfigResp>().await {
                    let dl_url = format!("{}/api/update/client/download", origin);
                    println!(
                        "[UPDATER CLOUD] Berhasil mengenali rilis Cloud via /api/update/config: v{}",
                        cfg_data.latest_version
                    );
                    return Ok(CloudManifest {
                        server_version: cfg_data.latest_version.clone(),
                        client_version: Some(cfg_data.latest_version),
                        release_notes: cfg_data.release_notes,
                        server_download_url: None,
                        client_download_url: Some(dl_url),
                        published_at: Some(chrono::Utc::now().to_rfc3339()),
                    });
                }
            }
        }

        // Panggil /api/update/check di Cloud Server jika /api/update/config belum berhasil
        let check_url = format!("{}/api/update/check?current_version=0.0.0", origin);
        let mut chk_req = client.get(&check_url);
        if !effective_key.is_empty() {
            chk_req = chk_req.header(header::AUTHORIZATION, format!("Bearer {}", effective_key));
        }

        if let Ok(res) = chk_req.send().await {
            if res.status().is_success() {
                if let Ok(update_info) = res.json::<UpdateInfo>().await {
                    let dl_url = if update_info.download_url.starts_with("http://") || update_info.download_url.starts_with("https://") {
                        update_info.download_url
                    } else {
                        format!("{}{}", origin, update_info.download_url)
                    };

                    println!(
                        "[UPDATER CLOUD] Berhasil mengenali rilis Cloud via /api/update/check: v{}",
                        update_info.latest_version
                    );
                    return Ok(CloudManifest {
                        server_version: update_info.latest_version.clone(),
                        client_version: Some(update_info.latest_version),
                        release_notes: Some(update_info.release_notes),
                        server_download_url: None,
                        client_download_url: Some(dl_url),
                        published_at: Some(chrono::Utc::now().to_rfc3339()),
                    });
                }
            }
        }
    }

    Err(format!(
        "Gagal mengambil manifest dari {} dan fallback endpoint Cloud juga tidak merespons.",
        manifest_url
    ))
}

#[allow(dead_code)]
pub async fn fetch_manifest_direct(manifest_url: &str) -> Result<CloudManifest, String> {
    fetch_manifest_with_auth(manifest_url, None).await
}

/// Memeriksa apakah kandidat versi lebih baru dibanding versi saat ini berdasarkan SemVer numerik
pub fn is_newer_version(candidate_ver: &str, current_ver: &str) -> bool {
    let parse_ver = |s: &str| -> Vec<u64> {
        let clean = s.trim().trim_start_matches(|c| c == 'v' || c == 'V');
        clean
            .split(&['.', '-', '_'][..])
            .filter_map(|part| part.parse::<u64>().ok())
            .collect::<Vec<u64>>()
    };

    let cand_parts = parse_ver(candidate_ver);
    let curr_parts = parse_ver(current_ver);

    if cand_parts.is_empty() || curr_parts.is_empty() {
        return candidate_ver.trim() != current_ver.trim();
    }

    let max_len = cand_parts.len().max(curr_parts.len());
    for i in 0..max_len {
        let c = cand_parts.get(i).copied().unwrap_or(0);
        let cur = curr_parts.get(i).copied().unwrap_or(0);
        if c > cur {
            return true;
        } else if c < cur {
            return false;
        }
    }

    false
}

/// Menyelesaikan URL relatif (misal '/dist/setup.exe' atau 'dist/setup.exe') terhadap base URL manifest Cloud
pub fn resolve_cloud_url(base_manifest_url: &str, target_url: &str) -> String {
    let target = target_url.trim();
    if target.is_empty() {
        return String::new();
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }

    if let Ok(base) = reqwest::Url::parse(base_manifest_url.trim()) {
        if target.starts_with('/') {
            if let Ok(joined) = base.join(target) {
                return joined.to_string();
            }
        } else {
            let with_slash = format!("/{}", target);
            if let Ok(joined) = base.join(&with_slash) {
                return joined.to_string();
            }
        }
        let origin = base.origin().ascii_serialization();
        if target.starts_with('/') {
            return format!("{}{}", origin, target);
        } else {
            return format!("{}/{}", origin, target);
        }
    }

    target.to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerCloudCheckResponse {
    pub success: bool,
    pub current_server_version: String,
    pub server_update_available: bool,
    pub latest_server_version: String,
    pub server_download_url: Option<String>,
    pub current_client_version: String,
    pub client_update_available: bool,
    pub latest_client_version: Option<String>,
    pub client_download_url: Option<String>,
    pub release_notes: String,
    pub published_at: Option<String>,
    pub manifest_url: String,
    pub status_message: String,
}

pub async fn check_server_cloud_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
) -> Response {
    let cfg = state.update_config.read().await.clone();
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &cfg.api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }

    let manifest_url = cfg.cloud_manifest_url.trim();
    if manifest_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "URL Manifest Cloud belum diatur di konfigurasi."
            })),
        )
            .into_response();
    }

    println!("[UPDATER CLOUD] Memeriksa versi rilis Cloud ke: {}", manifest_url);

    let manifest = match fetch_manifest_with_auth(manifest_url, Some(&cfg.api_key)).await {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Gagal mengambil manifest Cloud: {}", e)
                })),
            )
                .into_response();
        }
    };

    let cur_server_ver = crate::error_logger::SERVER_VERSION.to_string();
    let server_update_available = is_newer_version(&manifest.server_version, &cur_server_ver);

    let cur_client_ver = cfg.latest_version.clone();
    let client_update_available = manifest
        .client_version
        .as_deref()
        .map(|v| is_newer_version(v, &cur_client_ver))
        .unwrap_or(false);

    let status_message = if server_update_available && client_update_available {
        format!(
            "Ditemukan pembaruan Server (v{}) dan Klien (v{})!",
            manifest.server_version,
            manifest.client_version.as_deref().unwrap_or("-")
        )
    } else if server_update_available {
        format!("Ditemukan pembaruan Server v{}!", manifest.server_version)
    } else if client_update_available {
        format!(
            "Ditemukan pembaruan Klien v{}!",
            manifest.client_version.as_deref().unwrap_or("-")
        )
    } else {
        "Aplikasi Server & Klien Anda sudah menggunakan versi paling mutakhir.".to_string()
    };

    let resolved_server_url = manifest
        .server_download_url
        .as_deref()
        .map(|u| resolve_cloud_url(manifest_url, u));
    let resolved_client_url = manifest
        .client_download_url
        .as_deref()
        .map(|u| resolve_cloud_url(manifest_url, u));

    let resp = ServerCloudCheckResponse {
        success: true,
        current_server_version: cur_server_ver,
        server_update_available,
        latest_server_version: manifest.server_version,
        server_download_url: resolved_server_url,
        current_client_version: cur_client_ver,
        client_update_available,
        latest_client_version: manifest.client_version,
        client_download_url: resolved_client_url,
        release_notes: manifest.release_notes.unwrap_or_else(|| "Tidak ada catatan rilis.".to_string()),
        published_at: manifest.published_at,
        manifest_url: manifest_url.to_string(),
        status_message,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

#[derive(Deserialize)]
pub struct ApplyServerUpdatePayload {
    pub download_url: Option<String>,
}

pub async fn apply_server_cloud_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
    Json(payload): Json<ApplyServerUpdatePayload>,
) -> Response {
    let cfg = state.update_config.read().await.clone();
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &cfg.api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }

    let raw_download_url = match payload.download_url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => match fetch_manifest_with_auth(&cfg.cloud_manifest_url, Some(&cfg.api_key)).await {
            Ok(m) => match m.server_download_url {
                Some(u) if !u.trim().is_empty() => u,
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "success": false,
                            "error": "URL unduhan server tidak ditemukan di manifest cloud."
                        })),
                    )
                        .into_response();
                }
            },
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "success": false,
                        "error": format!("Gagal mengambil manifest cloud: {}", e)
                    })),
                )
                    .into_response();
            }
        },
    };

    let download_url = resolve_cloud_url(&cfg.cloud_manifest_url, &raw_download_url);
    println!("[UPDATER SERVER] Mengunduh binary server baru dari: {}", download_url);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal membuat HTTP client: {}", e) })),
            )
                .into_response();
        }
    };

    let effective_key = if !cfg.api_key.trim().is_empty() {
        cfg.api_key.trim()
    } else {
        DEFAULT_UPDATE_API_KEY
    };

    let res = match client
        .get(&download_url)
        .header(header::AUTHORIZATION, format!("Bearer {}", effective_key))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal mengunduh binary server: {}", e) })),
            )
                .into_response();
        }
    };

    if !res.status().is_success() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "success": false, "error": format!("Server download HTTP {}", res.status()) })),
        )
            .into_response();
    }

    let bytes = match res.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal membaca data unduhan: {}", e) })),
            )
                .into_response();
        }
    };

    if bytes.len() < 300_000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Ukuran file unduhan mencurigakan (hanya {} bytes), batalkan update.", bytes.len())
            })),
        )
            .into_response();
    }

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal menemukan path server aktif: {}", e) })),
            )
                .into_response();
        }
    };

    let temp_dir = std::env::temp_dir();
    let is_installer = download_url.to_lowercase().contains("setup")
        || bytes.windows(10).any(|w| w == b"Inno Setup");

    if is_installer {
        println!("[UPDATER SERVER] Terdeteksi paket installer setup ({} bytes). Menyiapkan eksekusi silent update...", bytes.len());
        let installer_path = temp_dir.join("MolayServer-Update-Setup.exe");
        if let Err(e) = std::fs::write(&installer_path, &bytes) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal menyimpan file installer ke {:?}: {}", installer_path, e) })),
            ).into_response();
        }

        let batch_path = temp_dir.join("molay_server_apply_installer.bat");
        let batch_content = format!(
            "@echo off\r\n\
             timeout /t 2 /nobreak >nul\r\n\
             taskkill /F /IM MolayServer-Sekar7.exe >nul 2>&1\r\n\
             start \"\" /wait \"{}\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART\r\n\
             if exist \"{}\" (\r\n\
                 start \"\" \"{}\"\r\n\
             )\r\n\
             del /f /q \"{}\" >nul 2>&1\r\n\
             del \"%~f0\"\r\n",
            installer_path.display(),
            current_exe.display(),
            current_exe.display(),
            installer_path.display()
        );

        if let Err(e) = std::fs::write(&batch_path, batch_content) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal menulis batch script updater: {}", e) })),
            ).into_response();
        }

        let _ = std::process::Command::new("cmd")
            .args(["/c", &batch_path.to_string_lossy()])
            .spawn();

        println!("[UPDATER SERVER] Batch installer silent update diluncurkan. Server me-restart dalam 800ms...");

        tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(800)).await;
            std::process::exit(0);
        });

        return Json(serde_json::json!({
            "success": true,
            "message": "Pembaruan server (installer) berhasil diunduh! Server sedang me-restart ke versi baru secara otomatis...",
            "reconnect_wait_seconds": 8
        })).into_response();
    }

    let new_exe = current_exe.with_extension("exe.new");
    let old_exe = current_exe.with_extension("exe.old");

    if let Err(e) = std::fs::write(&new_exe, &bytes) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Gagal menyimpan file binary baru ke {:?}: {}", new_exe, e) })),
        )
            .into_response();
    }

    println!("[UPDATER SERVER] Berhasil mengunduh {} bytes standalone binary. Menyiapkan restart otomatis...", bytes.len());

    let batch_path = temp_dir.join("molay_server_apply_update.bat");
    let batch_content = format!(
        "@echo off\r\n\
         timeout /t 2 /nobreak >nul\r\n\
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
        current_exe.display()
    );

    if let Err(e) = std::fs::write(&batch_path, batch_content) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Gagal menulis batch script updater: {}", e) })),
        )
            .into_response();
    }

    let _ = std::process::Command::new("cmd")
        .args(["/c", &batch_path.to_string_lossy()])
        .spawn();

    println!("[UPDATER SERVER] Batch updater diluncurkan. Server akan restart dalam 800ms...");

    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(800)).await;
        std::process::exit(0);
    });

    Json(serde_json::json!({
        "success": true,
        "message": "Pembaruan server berhasil diunduh! Server sedang me-restart ke versi baru secara otomatis...",
        "reconnect_wait_seconds": 6
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct SyncClientPayload {
    pub download_url: Option<String>,
    pub target_version: Option<String>,
    pub auto_broadcast: Option<bool>,
}

pub async fn sync_client_from_cloud(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<crate::auth::AuthQuery>,
    Json(payload): Json<SyncClientPayload>,
) -> Response {
    let mut cfg = state.update_config.read().await.clone();
    if crate::auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none()
        && !is_authorized(&headers, &cfg.api_key)
    {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
    }

    let (raw_download_url, target_ver, release_notes) = if let (Some(u), Some(v)) = (payload.download_url, payload.target_version) {
        (u, v, cfg.release_notes.clone())
    } else {
        match fetch_manifest_with_auth(&cfg.cloud_manifest_url, Some(&cfg.api_key)).await {
            Ok(m) => {
                let u = m.client_download_url.ok_or("URL unduhan klien tidak ada di manifest");
                let v = m.client_version.ok_or("Versi klien tidak ada di manifest");
                match (u, v) {
                    (Ok(url), Ok(ver)) => (url, ver, m.release_notes.unwrap_or(cfg.release_notes.clone())),
                    (Err(e), _) | (_, Err(e)) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({ "success": false, "error": e })),
                        )
                            .into_response();
                    }
                }
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "success": false, "error": format!("Gagal memuat manifest: {}", e) })),
                )
                    .into_response();
            }
        }
    };

    let download_url = resolve_cloud_url(&cfg.cloud_manifest_url, &raw_download_url);
    println!("[UPDATER SYNC] Mengunduh binary klien v{} dari {}", target_ver, download_url);

    let client = match reqwest::Client::builder().timeout(Duration::from_secs(180)).build() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": format!("Gagal membuat HTTP client: {}", e) })),
            ).into_response();
        }
    };

    let effective_key = if !cfg.api_key.trim().is_empty() {
        cfg.api_key.trim()
    } else {
        DEFAULT_UPDATE_API_KEY
    };

    // Klien di PC lab membutuhkan file binary langsung (bukan installer setup) untuk self-update
    let mut candidate_urls = vec![download_url.clone()];
    if download_url.contains("-Setup.exe") {
        candidate_urls.insert(0, download_url.replace("-Setup.exe", ".exe"));
    } else if download_url.contains("Setup.exe") {
        candidate_urls.insert(0, download_url.replace("Setup.exe", ".exe"));
    }

    let mut final_bytes = None;
    let mut last_err = String::new();

    for candidate in candidate_urls {
        let req = client
            .get(&candidate)
            .header(header::AUTHORIZATION, format!("Bearer {}", effective_key));
        match req.send().await {
            Ok(res) if res.status().is_success() => {
                if let Ok(b) = res.bytes().await {
                    if b.len() >= 300_000 {
                        final_bytes = Some(b);
                        break;
                    }
                }
            }
            Ok(res) => {
                last_err = format!("HTTP {}", res.status());
            }
            Err(e) => {
                last_err = format!("{}", e);
            }
        }
    }

    let bytes = match final_bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Gagal mengunduh binary klien: {}", last_err)
                })),
            ).into_response();
        }
    };

    if bytes.len() < 300_000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": "Ukuran file binary klien terlalu kecil (<300KB)." })),
        ).into_response();
    }

    let target_path = PathBuf::from(&cfg.client_binary_path);
    if let Some(parent) = target_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&target_path, &bytes) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Gagal menyimpan binary klien ke {:?}: {}", target_path, e) })),
        ).into_response();
    }

    // Pastikan salinan juga tersimpan di folder `updates/MolayClient-Sekar7.exe`
    // agar selalu ditemukan oleh download_client() saat klien LAN meminta pembaruan
    let _ = std::fs::create_dir_all("updates");
    let _ = std::fs::write("updates/MolayClient-Sekar7.exe", &bytes);

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let _ = std::fs::create_dir_all(exe_dir.join("updates"));
            let _ = std::fs::write(exe_dir.join("updates").join("MolayClient-Sekar7.exe"), &bytes);
            let _ = std::fs::write(exe_dir.join("MolayClient-Sekar7.exe"), &bytes);
        }
    }

    {
        let mut wcfg = state.update_config.write().await;
        wcfg.latest_version = target_ver.clone();
        wcfg.release_notes = release_notes.clone();
        let _ = save_update_config(&wcfg);
        cfg = wcfg.clone();
    }

    println!("[UPDATER SYNC] Binary klien v{} berhasil disimpan di {:?} ({} bytes)", target_ver, target_path, bytes.len());

    let mut broadcast_count = 0;
    if payload.auto_broadcast.unwrap_or(true) {
        let info = UpdateInfo {
            update_available: true,
            current_version: "current".to_string(),
            latest_version: target_ver.clone(),
            release_notes: release_notes.clone(),
            download_url: "/api/update/client/download".to_string(),
            sha256: None,
            mandatory: cfg.mandatory,
        };

        let senders = state.client_senders.read().await;
        for tx in senders.values() {
            let msg = ServerMessage::UpdateNotification { info: info.clone() };
            if tx.send(msg).is_ok() {
                broadcast_count += 1;
            }
        }
        println!("[UPDATER SYNC] Siaran update v{} otomatis dikirim ke {} klien online", target_ver, broadcast_count);
    }

    Json(serde_json::json!({
        "success": true,
        "message": format!("Binary klien v{} berhasil disinkronkan ke server lokal!", target_ver),
        "target_version": target_ver,
        "file_size": bytes.len(),
        "broadcast_sent_count": broadcast_count,
    })).into_response()
}

pub async fn get_cloud_manifest(State(state): State<AppState>) -> Response {
    let cfg = state.update_config.read().await;
    let manifest = CloudManifest {
        server_version: crate::error_logger::SERVER_VERSION.to_string(),
        client_version: Some(cfg.latest_version.clone()),
        release_notes: Some(cfg.release_notes.clone()),
        server_download_url: Some("/dist/MolayServer-Sekar7.exe".to_string()),
        client_download_url: Some("/api/update/client/download".to_string()),
        published_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    (StatusCode::OK, Json(manifest)).into_response()
}

pub fn start_background_cloud_checker(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;

        loop {
            let (should_check, manifest_url, api_key, auto_sync) = {
                let cfg = state.update_config.read().await;
                (cfg.auto_check_cloud, cfg.cloud_manifest_url.clone(), cfg.api_key.clone(), cfg.auto_sync_client)
            };

            if should_check && !manifest_url.trim().is_empty() {
                if let Ok(manifest) = fetch_manifest_with_auth(&manifest_url, Some(&api_key)).await {
                    let cur_server_ver = crate::error_logger::SERVER_VERSION;
                    let has_server_update = is_newer_version(&manifest.server_version, cur_server_ver);

                    if has_server_update {
                        println!(
                            "[UPDATER CLOUD] Terdeteksi versi server baru di Cloud: v{} (Versi lokal: v{})",
                            manifest.server_version, cur_server_ver
                        );

                        let resolved_url = manifest
                            .server_download_url
                            .as_deref()
                            .map(|u| resolve_cloud_url(&manifest_url, u));

                        let notify_json = serde_json::json!({
                            "type": "ServerUpdateNotification",
                            "current_version": cur_server_ver,
                            "latest_version": manifest.server_version,
                            "release_notes": manifest.release_notes.clone().unwrap_or_default(),
                            "server_download_url": resolved_url,
                        });
                        let _ = state.dashboard_tx.send(notify_json.to_string());
                    }

                    // Periksa rilis versi klien baru di Cloud
                    let cur_client_ver = state.update_config.read().await.latest_version.clone();
                    if let Some(ref cloud_client_ver) = manifest.client_version {
                        if is_newer_version(cloud_client_ver, &cur_client_ver) {
                            println!(
                                "[UPDATER CLOUD] Terdeteksi versi klien baru di Cloud: v{} (Versi target lokal saat ini: v{})",
                                cloud_client_ver, cur_client_ver
                            );

                            let notify_client = serde_json::json!({
                                "type": "ClientUpdateNotification",
                                "current_version": cur_client_ver,
                                "latest_version": cloud_client_ver,
                                "release_notes": manifest.release_notes.clone().unwrap_or_default(),
                                "client_download_url": manifest.client_download_url.as_deref().map(|u| resolve_cloud_url(&manifest_url, u)),
                            });
                            let _ = state.dashboard_tx.send(notify_client.to_string());

                            // Jika auto_sync_client aktif, otomatis unduh binary klien baru & siarkan ke PC lab!
                            if auto_sync {
                                println!("[UPDATER CLOUD] Auto-Sync aktif! Otomatis mengunduh binary klien v{} ke server lokal...", cloud_client_ver);
                                let download_url = manifest.client_download_url.as_deref().map(|u| resolve_cloud_url(&manifest_url, u));
                                if let Some(dl_url) = download_url {
                                    let dummy_headers = HeaderMap::new();
                                    let auth_query = crate::auth::AuthQuery { token: None, api_key: Some(api_key.clone()) };
                                    let sync_payload = SyncClientPayload {
                                        download_url: Some(dl_url),
                                        target_version: Some(cloud_client_ver.clone()),
                                        auto_broadcast: Some(true),
                                    };
                                    let _ = sync_client_from_cloud(
                                        State(state.clone()),
                                        dummy_headers,
                                        Query(auth_query),
                                        Json(sync_payload),
                                    ).await;
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_is_newer_version() {
        assert!(!is_newer_version("1.0.1", "1.0.2"));
        assert!(!is_newer_version("1.0.2", "1.0.2"));
        assert!(!is_newer_version("v1.0.2", "1.0.2"));
        assert!(is_newer_version("1.0.3", "1.0.2"));
        assert!(is_newer_version("1.1.0", "1.0.2"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(is_newer_version("1.0.2.1", "1.0.2"));
        assert!(!is_newer_version("1.0.2", "1.0.2.1"));
    }

    #[test]
    fn test_resolve_cloud_url() {
        let manifest = "https://molay.sekolahkartini7.web.id/api/cloud-manifest.json";
        assert_eq!(
            resolve_cloud_url(manifest, "/dist/MolayServer-Sekar7-Setup.exe"),
            "https://molay.sekolahkartini7.web.id/dist/MolayServer-Sekar7-Setup.exe"
        );
        assert_eq!(
            resolve_cloud_url(manifest, "dist/MolayServer-Sekar7-Setup.exe"),
            "https://molay.sekolahkartini7.web.id/dist/MolayServer-Sekar7-Setup.exe"
        );
        assert_eq!(
            resolve_cloud_url(manifest, "https://cdn.example.com/app.exe"),
            "https://cdn.example.com/app.exe"
        );
        assert_eq!(resolve_cloud_url(manifest, ""), "");
    }

    #[test]
    fn test_is_authorized_with_bearer_token() {
        let key = "cfut_EFE4TDO6DKe6tGvVKfgGgjW1Hldzn6ss43VLVg2J7b17463a";

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", key)).unwrap(),
        );

        assert!(is_authorized(&headers, key));

        // Token salah
        let mut wrong_headers = HeaderMap::new();
        wrong_headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str("Bearer token_palsu_123").unwrap(),
        );
        assert!(!is_authorized(&wrong_headers, key));

        // Tanpa token
        let empty_headers = HeaderMap::new();
        assert!(!is_authorized(&empty_headers, key));
    }

    #[test]
    fn test_is_authorized_with_custom_header() {
        let key = "cfut_EFE4TDO6DKe6tGvVKfgGgjW1Hldzn6ss43VLVg2J7b17463a";

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(key).unwrap());
        assert!(is_authorized(&headers, key));
    }

    #[test]
    fn test_cloud_manifest_deserialization() {
        let json_data = r#"{
            "server_version": "1.0.2",
            "client_version": "1.0.2",
            "release_notes": "Fitur baru pembaruan otomatis",
            "server_download_url": "https://example.com/MolayServer.exe",
            "client_download_url": "https://example.com/MolayClient.exe"
        }"#;

        let manifest: CloudManifest = serde_json::from_str(json_data).expect("Must parse valid manifest");
        assert_eq!(manifest.server_version, "1.0.2");
        assert_eq!(manifest.client_version.as_deref(), Some("1.0.2"));
        assert_eq!(manifest.server_download_url.as_deref(), Some("https://example.com/MolayServer.exe"));
        assert_eq!(manifest.client_download_url.as_deref(), Some("https://example.com/MolayClient.exe"));
    }

    #[test]
    fn test_update_config_backward_compatibility() {
        // JSON lama tanpa cloud_manifest_url dan auto_check_cloud
        let old_json = r#"{
            "latest_version": "1.0.0",
            "release_notes": "Rilis awal",
            "api_key": "secret",
            "mandatory": false,
            "client_binary_path": "dist/Molay-Client/MolayClient-Sekar7.exe"
        }"#;

        let cfg: UpdateConfig = serde_json::from_str(old_json).expect("Must parse legacy config format");
        assert_eq!(cfg.latest_version, "1.0.0");
        assert_eq!(cfg.cloud_manifest_url, default_cloud_manifest_url());
        assert!(cfg.auto_check_cloud);
        assert!(!cfg.auto_sync_client);
    }

    #[test]
    fn test_update_config_with_auto_sync() {
        let json_with_sync = r#"{
            "latest_version": "1.0.5",
            "release_notes": "Sinkronisasi otomatis LAN",
            "api_key": "secret123",
            "mandatory": true,
            "client_binary_path": "updates/MolayClient-Sekar7.exe",
            "cloud_manifest_url": "https://molay.sekolahkartini7.web.id/api/cloud-manifest.json",
            "auto_check_cloud": true,
            "auto_sync_client": true
        }"#;

        let cfg: UpdateConfig = serde_json::from_str(json_with_sync).expect("Must parse new config format");
        assert_eq!(cfg.latest_version, "1.0.5");
        assert!(cfg.auto_sync_client);
        assert!(cfg.mandatory);
    }

    #[test]
    fn test_is_authorized_with_default_key_fallback() {
        // Ketika server dikonfigurasi dengan custom key, request dengan default key tetap diterima
        let custom_key = "custom_super_secret_key_999";
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", DEFAULT_UPDATE_API_KEY)).unwrap(),
        );

        assert!(is_authorized(&headers, custom_key));
    }

    #[tokio::test]
    async fn test_fetch_manifest_fallback_to_live_cloud() {
        let manifest_url = "https://molay.sekolahkartini7.web.id/api/cloud-manifest.json";
        let res = fetch_manifest_with_auth(manifest_url, Some(DEFAULT_UPDATE_API_KEY)).await;
        assert!(res.is_ok(), "Fallback ke live cloud harus berhasil: {:?}", res.err());
        let m = res.unwrap();
        assert_eq!(m.server_version, "1.0.1");
        assert_eq!(m.client_version.as_deref(), Some("1.0.1"));
        assert!(m.client_download_url.is_some());
    }
}
