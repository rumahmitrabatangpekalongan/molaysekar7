//! Modul Manajemen Kunci API Layanan Eksternal & Sinkronisasi Workbuddy
//!
//! Mengelola persistent service API keys (M2M / Machine-to-Machine)
//! agar layanan seperti workbuddy.sekolahkartini7.web.id dapat mengakses
//! data analisis, log aktivitas, dan status lab secara stateless & tahan restart.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{auth, AppState};

pub const SERVICE_KEYS_FILE: &str = "service_keys.json";
pub const DEFAULT_WORKBUDDY_KEY: &str = "molay_sec_workbuddy_8f92a1e4c7b6d0e3";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceApiKey {
    pub key: String,
    pub name: String,
    pub role: String,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceKeyConfig {
    pub keys: Vec<ServiceApiKey>,
}

impl Default for ServiceKeyConfig {
    fn default() -> Self {
        Self {
            keys: vec![ServiceApiKey {
                key: DEFAULT_WORKBUDDY_KEY.to_string(),
                name: "Workbuddy AI Integration".to_string(),
                role: "admin".to_string(),
                enabled: true,
                description: "Kunci sinkronisasi data analisis & aktivitas untuk workbuddy.sekolahkartini7.web.id".to_string(),
                created_at: Utc::now().to_rfc3339(),
            }],
        }
    }
}

impl ServiceKeyConfig {
    pub fn find_valid_key(&self, token: &str) -> Option<&ServiceApiKey> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return None;
        }
        self.keys.iter().find(|k| k.enabled && k.key == trimmed)
    }
}

pub fn generate_secure_api_key(prefix: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    let secs = now.as_secs();
    let pid = std::process::id();

    let mut h1: u64 = 0xa5b3c4d5e6f70819;
    for b in format!("{}:{}:{}:{}", prefix, secs, nanos, pid).bytes() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);
    }
    let mut h2: u64 = 0x5b3c4d5e6f70819a;
    for b in format!("{}:{}:{}:{}", nanos, pid, prefix, secs).bytes() {
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    format!("molay_sec_{}_{:016x}{:016x}", prefix, h1, h2)
}

pub fn load_or_init_service_keys() -> ServiceKeyConfig {
    if let Ok(data) = std::fs::read_to_string(SERVICE_KEYS_FILE) {
        if let Ok(cfg) = serde_json::from_str::<ServiceKeyConfig>(&data) {
            println!("[API-SERVICE] Memuat {} API Key layanan dari {}", cfg.keys.len(), SERVICE_KEYS_FILE);
            return cfg;
        }
    }

    let default_cfg = ServiceKeyConfig::default();
    if let Ok(json) = serde_json::to_string_pretty(&default_cfg) {
        let _ = std::fs::write(SERVICE_KEYS_FILE, json);
        println!("[API-SERVICE] Inisialisasi API Key Workbuddy otomatis di {}", SERVICE_KEYS_FILE);
    }
    default_cfg
}

pub fn save_service_keys_to_file(config: &ServiceKeyConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Gagal serialisasi API Key: {}", e))?;
    std::fs::write(SERVICE_KEYS_FILE, json)
        .map_err(|e| format!("Gagal menulis {}: {}", SERVICE_KEYS_FILE, e))?;
    Ok(())
}

// Handlers:

/// Status sinkronisasi & verifikasi API Key
#[derive(Serialize)]
pub struct SyncStatusResponse {
    pub status: String,
    pub message: String,
    pub authenticated_as: String,
    pub role: String,
    pub server_version: String,
    pub server_time_utc: String,
    pub total_persisted_activities: usize,
    pub active_clients_count: usize,
    pub cloud_url: String,
    pub local_port: u16,
}

pub async fn sync_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
) -> Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    let session = match auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "error": "Unauthorized",
                    "message": "Kunci API tidak valid atau tidak disertakan. Gunakan header 'X-API-KEY: <KEY>' atau 'Authorization: Bearer <KEY>'."
                })),
            ).into_response();
        }
    };

    // Pastikan jika memory masih kosong, coba muat ulang dari file
    let activities_count = {
        let count = state.activities.read().await.len();
        if count == 0 {
            let fresh = crate::load_persisted_activities();
            let fresh_count = fresh.len();
            if fresh_count > 0 {
                let mut acts_w = state.activities.write().await;
                *acts_w = fresh;
            }
            fresh_count
        } else {
            count
        }
    };
    let clients_count = state.clients.read().await.len();

    (
        StatusCode::OK,
        Json(SyncStatusResponse {
            status: "ok".to_string(),
            message: "Koneksi ke Molay Server Sekar7 berhasil terverifikasi. Siap sinkronisasi data analisis.".to_string(),
            authenticated_as: session.full_name,
            role: session.role,
            server_version: crate::error_logger::SERVER_VERSION.to_string(),
            server_time_utc: Utc::now().to_rfc3339(),
            total_persisted_activities: activities_count,
            active_clients_count: clients_count,
            cloud_url: "https://molay.sekolahkartini7.web.id".to_string(),
            local_port: 1916,
        }),
    ).into_response()
}

/// Menampilkan daftar Service API Key (Khusus Admin/Guru)
pub async fn list_service_keys_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
) -> Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        ).into_response();
    }

    let cfg = state.service_keys.read().await;
    Json(cfg.clone()).into_response()
}

#[derive(Deserialize)]
pub struct RegenerateKeyPayload {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Membuat ulang kunci API Workbuddy atau menambahkan service baru
pub async fn regenerate_service_key_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(payload): Json<RegenerateKeyPayload>,
) -> Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        ).into_response();
    }

    let name = payload.name.unwrap_or_else(|| "Workbuddy AI Integration".to_string());
    let desc = payload.description.unwrap_or_else(|| "Kunci sinkronisasi data analisis & aktivitas untuk workbuddy.sekolahkartini7.web.id".to_string());
    let new_key_str = generate_secure_api_key("workbuddy");

    let mut cfg = state.service_keys.write().await;
    let mut updated = false;
    for k in &mut cfg.keys {
        if k.name == name {
            k.key = new_key_str.clone();
            k.enabled = true;
            k.description = desc.clone();
            k.created_at = Utc::now().to_rfc3339();
            updated = true;
            break;
        }
    }

    if !updated {
        cfg.keys.push(ServiceApiKey {
            key: new_key_str.clone(),
            name: name.clone(),
            role: "admin".to_string(),
            enabled: true,
            description: desc,
            created_at: Utc::now().to_rfc3339(),
        });
    }

    let _ = save_service_keys_to_file(&cfg);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "message": "API Key berhasil diperbarui!",
            "key": new_key_str,
            "name": name
        })),
    ).into_response()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_service_key_config_default() {
        let cfg = ServiceKeyConfig::default();
        assert_eq!(cfg.keys.len(), 1);
        let k = &cfg.keys[0];
        assert_eq!(k.key, DEFAULT_WORKBUDDY_KEY);
        assert_eq!(k.name, "Workbuddy AI Integration");
        assert_eq!(k.role, "admin");
        assert!(k.enabled);
    }

    #[test]
    fn test_find_valid_key() {
        let cfg = ServiceKeyConfig::default();
        assert!(cfg.find_valid_key(DEFAULT_WORKBUDDY_KEY).is_some());
        assert!(cfg.find_valid_key("wrong_key_123").is_none());
        assert!(cfg.find_valid_key("").is_none());
    }

    #[test]
    fn test_generate_secure_api_key() {
        let k1 = generate_secure_api_key("workbuddy");
        let k2 = generate_secure_api_key("workbuddy");
        assert!(k1.starts_with("molay_sec_workbuddy_"));
        assert!(!k1.is_empty());
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_disabled_key_ignored() {
        let mut cfg = ServiceKeyConfig::default();
        cfg.keys[0].enabled = false;
        assert!(cfg.find_valid_key(DEFAULT_WORKBUDDY_KEY).is_none());
    }

    #[test]
    fn test_find_valid_key_with_whitespace() {
        let cfg = ServiceKeyConfig::default();
        let key_with_spaces = format!("  {}  ", DEFAULT_WORKBUDDY_KEY);
        assert!(cfg.find_valid_key(&key_with_spaces).is_some());
    }

    #[test]
    fn test_service_key_serialization_roundtrip() {
        let cfg = ServiceKeyConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: ServiceKeyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, deserialized);
    }
}
